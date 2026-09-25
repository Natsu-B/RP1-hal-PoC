#!/usr/bin/env python3
"""Compile synthetic DTBs; mutate actual candidate inputs, not golden reports."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from clock_profile import load, outputs
from validate_linux_dtb import Tree, decode, validate
from scmi_elf_layout import read_layout, transport_dtsi


class CandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.profile = load()
        cls.directory = tempfile.TemporaryDirectory(prefix="rp1-dtb-host-")
        cls.addClassCleanup(cls.directory.cleanup)
        cls.path = Path(cls.directory.name)
        generated = next(v for p, v in outputs(cls.profile).items() if p.name == "clocks.dtsi")
        # Synthetic address map ONLY. Not a hardware deployment DT.
        cls.source = '''/dts-v1/;
/ {
 #address-cells = <2>; #size-cells = <2>;
 reserved-memory { #address-cells = <2>; #size-cells = <2>; ranges;
  ddr: transport@21000000 { reg = <0 0x21000000 0 0x10000>; no-map; };
 };
 firmware {
  rp1_scmi: scmi { compatible = "arm,scmi"; #address-cells = <1>; #size-cells = <0>;
   mboxes = <&mbox 1>; shmem = <&scmi_mem>;
  };
 };
 rp1 { compatible = "simple-bus"; #address-cells = <2>; #size-cells = <2>; ranges;
  mbox: mailbox@8000 { compatible = "raspberrypi,rp1-mbox"; reg = <0 0x8000 0 0x100>; #mbox-cells = <1>; };
  sram@20000000 { compatible = "mmio-sram"; reg = <0 0x20000000 0 0x10000>;
   #address-cells = <1>; #size-cells = <1>; ranges = <0 0 0x20000000 0x10000>;
   scmi_mem: scmi@d000 { compatible = "arm,scmi-shmem"; reg = <0xd000 0x100>; };
  };
  rp1_clocks: clocks@18000 { compatible = "raspberrypi,rp1-clocks"; #clock-cells = <1>; status = "disabled"; };
  rp1_gpio: gpio@d0000 { compatible = "raspberrypi,rp1-gpio"; gpio-controller; #gpio-cells = <2>;
   uart_pins: uart1 { pins = "gpio0", "gpio1"; function = "uart1"; };
   cam1_pins: cam1 { pins = "gpio0", "gpio1"; function = "i2c0"; };
  };
  serial@30000 { status = "disabled"; clocks = <&rp1_clocks 15>; };
  spi@50000 { status = "disabled"; };
  i2c@74000 { status = "disabled"; };
  uart1: serial@34000 { compatible = "arm,pl011-axi"; status = "okay";
   clocks = <&rp1_fixed_uart>; clock-names = "uartclk";
   pinctrl-names = "default"; pinctrl-0 = <&uart_pins>;
  };
  cam1: i2c@70000 { status = "disabled"; pinctrl-0 = <&cam1_pins>; };
 };
};
''' + generated

    def candidate(self, suffix="", source=None, layout=None):
        src, dtb = self.path / "test.dts", self.path / "test.dtb"
        src.write_text((self.source if source is None else source) + suffix)
        subprocess.run(["dtc", "-I", "dts", "-O", "dtb", "-o", str(dtb), str(src)],
                       capture_output=True, check=True)
        return validate(decode(dtb), self.profile, firmware_layout=layout)

    def elf(self, digest=None, section=".scmi_shmem", size=256):
        # Small REAL ARM ELF, not a JSON report pretending to be linked firmware.
        src, obj, link, elf = [self.path / ('layout.' + s) for s in ('s', 'o', 'ld', 'elf')]
        src.write_text(f'''.syntax unified
.thumb
.section .text,"ax"
.global Reset
.thumb_func
Reset: b Reset
.section .bss,"aw",%nobits
.space 256
.section .rp1_clock_profile,"a"
.ascii "{digest or self.profile['sha256']}"
.section {section},"aw",%nobits
.balign 64
.space {size}
''')
        link.write_text('''ENTRY(Reset)
MEMORY { SRAM (rwx) : ORIGIN = 0x20000000, LENGTH = 56K }
SECTIONS {
 .text : { *(.text) } > SRAM
 .bss (NOLOAD) : { *(.bss); __ebss = .; } > SRAM
 .rp1_clock_profile : { *(.rp1_clock_profile); } > SRAM
 .scmi_shmem (NOLOAD) : ALIGN(64) {
  __scmi_shmem_start = .; *(.scmi_shmem); __scmi_shmem_end = .;
 } > SRAM
 __image_end = .; __app_limit = 0x2000e000;
}
''')
        subprocess.run(['arm-none-eabi-as', '-mcpu=cortex-m3', '-mthumb', str(src), '-o', str(obj)], check=True, capture_output=True)
        subprocess.run(['arm-none-eabi-ld', '-T', str(link), str(obj), '-o', str(elf)], check=True, capture_output=True)
        return read_layout(elf)

    def linked_source(self, layout):
        # Synthetic flat parent bus. Real board PCI translation is checked
        # separately against the supplied base/final DTBs, never this fixture.
        return (self.source.replace('reg = <0 0x20000000 0 0x10000>', 'reg = <0xc0 0x40400000 0 0x10000>')
                .replace('ranges = <0 0 0x20000000 0x10000>', 'ranges = <0 0xc0 0x40400000 0x10000>')
                .replace('reg = <0xd000 0x100>', f'reg = <0x{layout["bar2_offset"]:x} 0x100>') +
                f'/ {{ rp1_firmware_layout {{ elf-sha256 = "{layout["elf_sha256"]}"; }}; }};')

    def bad(self, suffix, expected):
        result = self.candidate(suffix)
        self.assertEqual(result["result"], "FAIL")
        self.assertTrue(any(expected in f for f in result["failures"]), result["failures"])

    def test_complete_synthetic_candidate(self):
        r = self.candidate()
        self.assertEqual(r["failures"], [])
        self.assertEqual(r["result"], "PASS")
        self.assertTrue(r["remaining_admission"])
        self.assertIn("/rp1/serial@30000", r["disabled_nodes"])

    def test_clocks_child_not_property(self):
        r = self.candidate('/ { clocks { compatible = "simple-bus"; }; firmware { clocks { compatible = "raspberrypi,firmware-clocks"; }; }; };')
        self.assertEqual(r['failures'], [])

    def test_compiler_metadata_not_device(self):
        r = self.candidate('/ { __symbols__ { clocks = "/clocks"; }; __overrides__ { clocks = <0xdead>; }; __fixups__ { clocks = "test"; }; __local_fixups__ { clocks = <0>; }; };')
        self.assertEqual(r['failures'], [])
        self.assertNotIn('/__symbols__', r['enabled_nodes'])

    def test_dynamic_cma_is_not_fixed_region(self):
        r = self.candidate('/ { reserved-memory { linux,cma { compatible = "shared-dma-pool"; size = <0 0x4000000>; alloc-ranges = <0 0 0 0x40000000>; reusable; linux,cma-default; }; }; };')
        self.assertEqual(r['failures'], [])
        cma = r['dynamic_reserved_memory'][0]
        self.assertIsNone(cma['physical_start'])
        self.assertEqual(cma['allocation_ranges'], [{'cpu_start': 0, 'size': 0x40000000}])
        self.assertIn('live dynamic reserved-memory allocations', r['remaining_admission'])

    def test_bad_dynamic_cma_cells(self):
        self.bad('/ { reserved-memory { cma { size = <0x4000000>; }; }; };', 'dynamic reservation size')

    def test_bad_dynamic_cma_ranges(self):
        self.bad('/ { reserved-memory { cma { size = <0 0x4000000>; alloc-ranges = <0 0 0x40000000>; }; }; };', 'dynamic allocation ranges')

    def test_bad_dynamic_cma_alignment(self):
        self.bad('/ { reserved-memory { cma { size = <0 0x4000000>; alignment = <0 3>; }; }; };', 'dynamic reservation alignment')

    def test_bad_dynamic_cma_flags(self):
        self.bad('/ { reserved-memory { cma { size = <0 0x4000000>; reusable; no-map; }; }; };', 'reusable and no-map')

    def test_clock_reference(self):
        self.bad('&uart1 { clocks = <&rp1_clocks 15>; };', 'references clk-rp1')

    def test_assigned_clock_reference(self):
        self.bad('&uart1 { assigned-clocks = <&rp1_clocks 15>; };', 'references clk-rp1')

    def test_assigned_parent_reference(self):
        self.bad('&uart1 { assigned-clock-parents = <&rp1_clocks 15>; };', 'references clk-rp1')

    def test_missing_provider(self):
        self.bad('&uart1 { clocks = <0x12345678>; };', 'unresolved clocks')

    def test_fixed_rate(self):
        self.bad('&rp1_fixed_uart { clock-frequency = <48000000>; };', 'fixed clock/profile mismatch')

    def test_bad_scmi_id(self):
        self.bad('&uart1 { clocks = <&rp1_fixed_uart>, <&rp1_scmi_clocks 99>; };', 'SCMI clock ID mismatch')

    def test_private_uart(self):
        self.bad('/ { rp1 { serial@30000 { status = "okay"; }; }; };', 'firmware-owned')

    def test_private_spi(self):
        self.bad('/ { rp1 { spi@50000 { status = "okay"; }; }; };', 'firmware-owned')

    def test_private_i2c(self):
        self.bad('/ { rp1 { i2c@74000 { status = "okay"; }; }; };', 'firmware-owned')

    def test_cts(self):
        self.bad('&uart1 { uart-has-rtscts; };', 'no CTS/RTS')

    def test_firmware_pin(self):
        self.bad('&uart_pins { pins = "gpio2", "gpio3"; };', 'firmware pins')

    def test_cam1_conflict(self):
        self.bad('&cam1 { status = "okay"; };', 'pin ownership overlap')

    def test_reserved_overlap(self):
        self.bad('/ { reserved-memory { another@21001000 { reg = <0 0x21001000 0 0x1000>; no-map; }; }; };', 'shared/reserved overlap')

    def test_sram_overlap(self):
        self.bad('/ { rp1 { sram@20000000 { debug@d000 { reg = <0xd000 0x100>; }; }; }; };', 'shared/reserved overlap')

    def test_untranslatable_memory(self):
        self.bad('/ { rp1 { sram@20000000 { /delete-property/ ranges; }; }; };', 'missing address translation')

    def test_channel_mismatch(self):
        self.bad('&rp1_scmi { mboxes = <&mbox 2>; };', 'one bidirectional mailbox')

    def test_channel_reuse(self):
        self.bad('/ { another-client { mboxes = <&mbox 1>; }; };', 'mailbox channel also owned')

    def test_gpio_consumer_firmware_pin(self):
        self.bad('/ { sensor { reset-gpios = <&rp1_gpio 14 0>; }; };', 'firmware pins')

    def test_gpio_hog_firmware_pin(self):
        self.bad('&rp1_gpio { reserved-pin { gpio-hog; gpios = <2 0>; output-low; }; };', 'firmware pins')

    def test_gpio_consumer_uart_pin(self):
        self.bad('/ { sensor { reset-gpios = <&rp1_gpio 0 0>; }; };', 'pin ownership overlap')

    def test_wrong_uart_function(self):
        self.bad('&uart_pins { function = "i2c0"; };', 'pin function must be uart1')

    def test_bad_assigned_scmi_rate(self):
        self.bad('&uart1 { assigned-clocks = <&rp1_scmi_clocks 0>; assigned-clock-rates = <50000000>; };', 'assigned SCMI rate outside profile')

    def test_profile_digest(self):
        self.bad('/ { rp1_clock_profile { profile-sha256 = "bad"; }; };', 'SHA mismatch')

    def test_status_inheritance(self):
        self.bad('/ { rp1 { status = "disabled"; }; };', 'UART1 required')

    def test_camera_required(self):
        self.candidate()
        r = validate(decode(self.path / "test.dtb"), self.profile, True)
        self.assertIn('camera required but no enabled RP1 CFE', r["failures"])

    def test_generated_outputs(self):
        for p, text in outputs(self.profile).items():
            self.assertEqual(p.read_text(), text, str(p))

    def test_linked_layout_match(self):
        layout = self.elf()
        result = self.candidate(source=self.linked_source(layout), layout=layout)
        self.assertEqual(result['failures'], [])
        self.assertEqual(result['firmware_match']['scmi_cpu_start'], 0xc040400000 + layout['bar2_offset'])
        self.assertNotIn('firmware ELF SRAM placement/guards', result['remaining_admission'])
        self.assertIn('live BAR/CPU translation', result['remaining_admission'])

    def test_linked_bad_size(self):
        with self.assertRaisesRegex(ValueError, 'size/alignment'):
            self.elf(size=128)

    def test_linked_wrong_section(self):
        with self.assertRaisesRegex(ValueError, 'bounds'):
            self.elf(section='.not_scmi')

    def test_linked_old_profile(self):
        layout = self.elf(digest='0' * 64)
        r = self.candidate(source=self.linked_source(layout), layout=layout)
        self.assertIn('linked ELF profile SHA mismatch', r['failures'])

    def test_linked_dtb_old_elf(self):
        layout = self.elf()
        r = self.candidate(source=self.linked_source(layout).replace(layout['elf_sha256'], '0' * 64), layout=layout)
        self.assertIn('DT firmware ELF SHA mismatch/missing', r['failures'])

    def test_linked_wrong_slot(self):
        layout = self.elf()
        r = self.candidate('&scmi_mem { reg = <0xfb00 0x100>; };', self.linked_source(layout), layout)
        self.assertIn('SCMI DT placement does not match final ELF BAR2 reservation', r['failures'])

    def test_linked_wrong_sram_domain(self):
        layout = self.elf()
        r = self.candidate(source=self.linked_source(layout).replace('0x40400000', '0x40000000'), layout=layout)
        self.assertTrue(any('SRAM reg' in f for f in r['failures']), r)

    def test_linked_disagreeing_ranges(self):
        layout = self.elf()
        r = self.candidate(source=self.linked_source(layout).replace('ranges = <0 0xc0 0x40400000', 'ranges = <0 0xc0 0x40401000'), layout=layout)
        self.assertIn('SRAM child ranges disagree with parent reg', r['failures'])

    def test_linked_other_firmware_region(self):
        layout = self.elf()
        r = self.candidate('/ { rp1 { sram@20000000 { debug@fc00 { reg = <0xfc00 0x300>; }; }; }; };', self.linked_source(layout), layout)
        self.assertTrue(any('firmware-owned SRAM' in f for f in r['failures']), r)

    def test_transport_fragment(self):
        layout = self.elf()
        base = '''/dts-v1/; / {
 #address-cells = <2>; #size-cells = <2>;
 rp1 { #address-cells = <2>; #size-cells = <2>;
  ranges = <0xc0 0x40400000 0x1f 0x00400000 0 0x10000>;
  sram { compatible = "mmio-sram"; reg = <0xc0 0x40400000 0 0x10000>;
   #address-cells = <1>; #size-cells = <1>; ranges = <0 0xc0 0x40400000 0x10000>;
  };
  mailbox { compatible = "raspberrypi,rp1-mbox"; #mbox-cells = <1>; status = "disabled"; };
 }; };
'''
        self.candidate(source=base)
        tree = Tree(decode(self.path/'test.dtb'))
        fragment = transport_dtsi(layout, tree, self.profile)
        self.candidate(source=base + fragment)
        actual = Tree(decode(self.path/'test.dtb'))
        shared = f'/rp1/sram/scmi@{layout["bar2_offset"]:x}'
        self.assertTrue(actual.enabled['/rp1/mailbox'])
        self.assertEqual(actual.regs(shared), [(layout['bar2_offset'], 256)])
        self.assertEqual(actual.physical('/rp1/sram', layout['bar2_offset'], 256), 0x1f00400000 + layout['bar2_offset'])
        with self.assertRaisesRegex(ValueError, 'already has SCMI'):
            transport_dtsi(layout, actual, self.profile)


if __name__ == "__main__":
    unittest.main()
