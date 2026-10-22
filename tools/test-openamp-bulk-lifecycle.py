#!/usr/bin/env python3
"""Execute the actual C lifecycle bodies with a host-only state/endpoint model."""
import argparse
from pathlib import Path
import subprocess


def function(source, name):
    start = source.index(name + '(')
    start = source.rfind('\n', 0, start) + 1
    begin = source.index('{', start)
    depth = 1
    end = begin + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False)
    source = (Path(__file__).resolve().parents[1] / 'openamp/rp1.c').read_text()
    start = function(source, 'rp1_openamp_start')
    assert 'clear_transport();' in start
    assert all(name not in start for name in ('rp1_openamp_quiesce(', 'rp1_bulk_quiesce(', 'rp1_bulk_cold_reset('))
    bodies = '\n'.join(function(source, name) for name in (
        'rp1_openamp_cold_reset', 'clear_transport', 'rp1_openamp_quiesce'))
    test = '''#include <assert.h>
#include <stdint.h>
#include <string.h>
#define RP1_AMP_BULK 1
static struct { uint32_t online, last_sequence, pending; } state;
static int retired;
static void rp1_bulk_cold_reset(void) { retired=0; }
static void rp1_bulk_quiesce(void) { retired=1; }
''' + bodies + '''
int main(void) {
    rp1_openamp_cold_reset();
    state.last_sequence=7; state.online=1; state.pending=9;
    clear_transport();
    assert(!retired && !state.online && !state.pending && state.last_sequence==7);
    rp1_openamp_quiesce();
    assert(retired);
    clear_transport();
    assert(retired && state.last_sequence==7);
    rp1_openamp_cold_reset();
    assert(!retired && !state.last_sequence);
}
'''
    path = args.output / 'lifecycle.c'
    path.write_text(test)
    binary = args.output / 'lifecycle'
    subprocess.run(['gcc', '-std=c11', '-Wall', '-Wextra', '-Werror', str(path), '-o', str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
    print('HOST_PASS: first attach preserves admission; quiescence remains retired; only cold reset reopens')


if __name__ == '__main__':
    main()
