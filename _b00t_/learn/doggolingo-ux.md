---
defensive autoload guards: always use get_node_or_null() to reference singletons; null-check before accessing autoload properties. When referencing global constants like Theme.C_PRIMARY, cache the node reference first and use t.C_PRIMARY rather than direct global access. This prevents loading hangs when autoload initialization ordering fails on different platform export targets.
