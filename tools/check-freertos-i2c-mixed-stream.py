#!/usr/bin/env python3
"""ICMS 36-sample bounded stream; requires an independent external causal join."""
import json
from pathlib import Path
import runpy
import sys

PAIR=runpy.run_path(str(Path(__file__).with_name('check-freertos-i2c-mixed-pair.py')))
def validate(text):
    return PAIR['validate'](text,stream=True)

if __name__=='__main__':
    print(json.dumps(validate(Path(sys.argv[1]).read_text(errors='replace')),indent=2))
