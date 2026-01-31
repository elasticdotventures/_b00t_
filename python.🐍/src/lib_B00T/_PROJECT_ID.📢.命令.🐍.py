
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
