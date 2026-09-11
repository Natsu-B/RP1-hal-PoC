#!/usr/bin/env python3
"""Run the actual finite mixed-workload arithmetic, without hardware."""
from pathlib import Path
import subprocess
import tempfile

source=Path(__file__).resolve().parents[1]/'examples/minimal/src/mixed_repeat.rs'
with tempfile.TemporaryDirectory(prefix='rp1-mixed-repeat-') as directory:
    binary=Path(directory)/'test'
    subprocess.run(['rustc','+stable','--edition=2024','--test',str(source),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True)
print('STATIC actual sequence/cadence/token/wrap checks; no hardware')
