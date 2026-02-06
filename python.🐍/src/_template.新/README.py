#!/usr/bin/python

# DocOpt syntax https://pypi.org/project/docopt/
# Logging Cookbook: https://docs.python.org/3/howto/logging-cookbook.html#logging-cookbook

"""Naval Fate.

Usage:
  naval_fate.py ship new <name>...
  naval_fate.py ship <name> move <x> <y> [--speed=<kn>]
  naval_fate.py ship shoot <x> <y>
  naval_fate.py mine (set|remove) <x> <y> [--moored | --drifting]
  naval_fate.py (-h | --help)
  naval_fate.py --version

Options:
  -h --help     Show this screen.
  --version     Show version.
  --speed=<kn>  Speed in knots [default: 10].
  --moored      Moored (anchored) mine.
  --drifting    Drifting mine.

"""



""" 命令 Mìnglìng :: Command

Usage:
    # read from stdin
    命令 --stdin 

Options:
    -h --help  Show this Screen
    --version
    --length

## 命令 * * * \\
from docopt import docopt

def main():
    stdin_lines = []
    for line in sys.stdin:
        if line.strip() != "":
            stdin_lines.append(line.strip())

    # ... My code here ...
    
if __name__ == '__main__':
    main()
