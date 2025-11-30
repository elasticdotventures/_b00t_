#!/usr/bin/env python3
import argparse
from pathlib import Path
import sys

DOC_PATH = Path('docs/ANSIBLE_DATUM_QUEUE.md')
STATUS_VALUES = {'pending', 'in progress', 'done'}


def read_table():
    lines = DOC_PATH.read_text().splitlines()
    header_idx = next((i for i, line in enumerate(lines) if line.startswith('| Datum')), None)
    if header_idx is None:
        raise SystemExit('Could not find table header in ANSIBLE_DATUM_QUEUE.md')
    delim_idx = header_idx + 1
    rows = []
    for line in lines[delim_idx + 1:]:
        if not line.startswith('| '):
            break
        cells = [cell.strip() for cell in line.strip().strip('|').split('|')]
        if len(cells) != 4:
            continue
        rows.append({
            'line': line,
            'name': cells[0],
            'type': cells[1],
            'status': cells[2],
            'notes': cells[3],
        })
    suffix_start = delim_idx + 1 + len(rows)
    return lines[:header_idx], lines[header_idx], lines[delim_idx], rows, lines[suffix_start:]


def write_table(prefix, header, delim, rows, suffix):
    with DOC_PATH.open('w') as fh:
        for chunk in (prefix, [header, delim]) + ([row['line'] for row in rows], suffix):
            for line in chunk:
                fh.write(line + '\n')


def list_entries(rows):
    for row in rows:
        print(f"{row['name']}: {row['status']} ({row['notes']})")


def next_entry(rows):
    for row in rows:
        if row['status'].lower().startswith('pending'):
            print(f"Next datum to convert: {row['name']} ({row['type']}) - {row['notes']}")
            return
    print('All datums marked as done or in progress.')


def mark_entry(rows, name, status, notes):
    updated = False
    status_title = status.title()
    if status_title.lower() not in STATUS_VALUES:
        raise SystemExit(f"Status must be one of: {', '.join(STATUS_VALUES)}")
    for row in rows:
        if row['name'] == name:
            row['status'] = status_title
            if notes is not None:
                row['notes'] = notes
            row['line'] = f"| {row['name']} | {row['type']} | {row['status']} | {row['notes']} |"
            updated = True
            break
    if not updated:
        raise SystemExit(f"Datum '{name}' not found in queue")


def main():
    parser = argparse.ArgumentParser(description='Track ansible datum conversion status')
    subparsers = parser.add_subparsers(dest='cmd', required=True)

    subparsers.add_parser('list', help='List all datums and their status')
    subparsers.add_parser('next', help='Show the next pending datum')
    mark = subparsers.add_parser('mark', help='Update a datum status')
    mark.add_argument('name')
    mark.add_argument('status', help='Pending, In Progress, Done')
    mark.add_argument('--note', help='Optional note to attach')

    args = parser.parse_args()
    prefix, header, delim, rows, suffix = read_table()

    if args.cmd == 'list':
        list_entries(rows)
    elif args.cmd == 'next':
        next_entry(rows)
    elif args.cmd == 'mark':
        mark_entry(rows, args.name, args.status, args.note)
        write_table(prefix, header, delim, rows, suffix)
    else:
        parser.print_help()


if __name__ == '__main__':
    main()
