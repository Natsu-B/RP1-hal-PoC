#!/usr/bin/env python3
"""Compile synthetic DTBs; mutate actual candidate inputs, not golden reports."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from clock_profile import load, outputs
from validate_linux_dtb import decode, validate


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

    def candidate(self, suffix="", source=None):
        src, dtb = self.path / "test.dts", self.path / "test.dtb"
        src.write_text((self.source if source is None else source) + suffix)
        subprocess.run(["dtc", "-I", "dts", "-O", "dtb", "-o", str(dtb), str(src)],
                       capture_output=True, check=True)
        return validate(decode(dtb), self.profile)

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


if __name__ == "__main__":
    unittest.main()
