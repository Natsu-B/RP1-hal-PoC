import os
from pathlib import Path
import runpy
import unittest

check=runpy.run_path(str(Path(__file__).with_name('check-local-queue-codegen.py')))['check_disassembly']

class CodegenTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dis=Path(os.environ['RP1_LOCAL_QUEUE_DISASSEMBLY']).read_text()
    def test_actual_target_probe(self):
        self.assertEqual(check(self.dis)['queue_dmb_instructions'],4)
    def test_missing_barrier_rejected(self):
        with self.assertRaises(ValueError):check(self.dis.replace('dmb','nop',1))
    def test_exclusive_instruction_rejected(self):
        with self.assertRaises(ValueError):check(self.dis.replace('dmb','ldrex',1))
    def test_unadmitted_call_or_missing_symbol_rejected(self):
        for dis in [self.dis.replace('__aeabi_memcpy4','unadmitted_mmio'),
                    self.dis.replace('<local_queue_codegen_probe>','<unknown>'),
                    self.dis.replace('dmb','blx',1),
                    self.dis.replace('R_ARM_THM_CALL\t__aeabi_memcpy4','R_ARM_THM_JUMP24\tunadmitted_mmio')]:
            with self.assertRaises(ValueError):check(dis)

if __name__=='__main__':unittest.main()
