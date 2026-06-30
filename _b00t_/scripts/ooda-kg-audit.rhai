// ooda-kg-audit.rhai — OODA loop for knowledge graph topology audit
// Finds isolated nodes, analyzes graph shape, recommends connections.
// Run: b00t exec "rhai _b00t_/scripts/ooda-kg-audit.rhai"

let PROJECT = "_b00t_";
let MIN_DEGREE_FOR_LAYOUT = 5;   // nodes with >= this many edges get pushed apart
let ISOLATED_DEGREE = 0;          // 0 adjacencies = isolated

// ── OBSERVE: Find isolated nodes ──────────────────────────────────────────
fn observe_isolates() {
    // Query for nodes with 0 adjacencies across all labels
    let cypher = `
        MATCH (n)
        WHERE NOT (n)--()
        RETURN n.qualified_name AS name, labels(n) AS labels, n.file_path AS file
        LIMIT 20
    `;
    
    print("🔍 OBSERVE: finding isolated nodes (degree = 0)...");
    // This would call codebase-memory query_graph via MCP
    // For now, document the query pattern
    print(`  Cypher: ${cypher}`);
}

// ── OBSERVE: Analyze degree distribution ──────────────────────────────────
fn observe_degree_distribution() {
    let cypher = `
        MATCH (n)-[r]-()
        RETURN n.qualified_name AS name, labels(n) AS labels, count(r) AS degree
        ORDER BY degree DESC
        LIMIT 15
    `;
    
    print("📊 OBSERVE: degree distribution (top 15)...");
    print(`  Cypher: ${cypher}`);
}

// ── OBSERVE: Find high-degree hubs ────────────────────────────────────────
fn observe_hubs(min_degree) {
    let cypher = `
        MATCH (n)-[r]-()
        WITH n, count(r) AS degree
        WHERE degree >= ${min_degree}
        RETURN n.qualified_name AS name, labels(n) AS labels, degree
        ORDER BY degree DESC
    `;
    
    print(`🏗️  OBSERVE: hubs (degree >= ${min_degree})...`);
    print(`  Cypher: ${cypher}`);
}

// ── ORIENT: Cassowary-inspired layout heuristic ───────────────────────────
// Nodes with high degree SHOULD push away from other nodes.
// This is a heuristic — the actual layout engine runs in cytoscape.js
fn orient_layout_force(degree, max_degree, base_radius) {
    // Cassowary constraint: force ∝ degree / max_degree
    // Higher degree → stronger repulsion → larger spread in layout
    let force_ratio = degree / max_degree;
    let spread = base_radius * (1.0 + force_ratio * 2.0);
    return spread;
}

// ── ORIENT: Cluster detection heuristic ───────────────────────────────────
fn orient_clusters() {
    let cypher = `
        MATCH (n1)-[r]-(n2)
        WHERE n1.qualified_name < n2.qualified_name
        RETURN n1.qualified_name AS source, type(r) AS rel, n2.qualified_name AS target
        LIMIT 50
    `;
    print("🧩 ORIENT: cluster detection...");
    print(`  Cypher: ${cypher}`);
}

// ── DECIDE: Classify isolated node for reconnection ───────────────────────
// Rules-based classifier (finetuned model will replace this)
fn decide_reconnect(node_name, node_labels, context_hubs) {
    // Rule 1: Function nodes should connect to their callers/callees
    if node_labels.contains("Function") || node_labels.contains("Method") {
        return {
            action: "trace_calls",
            confidence: 0.8,
            reason: `Function '${node_name}' should have caller/callee edges`
        };
    }
    // Rule 2: Route nodes should have HTTP/async edges
    if node_labels.contains("Route") || node_labels.contains("HttpRoute") {
        return {
            action: "trace_routes",
            confidence: 0.9,
            reason: `Route '${node_name}' should have HTTP/async edges`
        };
    }
    // Rule 3: Module nodes without edges should be traced for imports
    if node_labels.contains("Module") || node_labels.contains("File") {
        return {
            action: "trace_imports",
            confidence: 0.6,
            reason: `Module '${node_name}' should trace imports`
        };
    }
    // Default: flag for manual review
    return {
        action: "flag",
        confidence: 0.3,
        reason: `Unknown node type: ${node_labels}`
    };
}

// ── ACT: Reconnect isolated nodes ─────────────────────────────────────────
fn act_reconnect(node_name, decision) {
    print(`🔧 ACT: ${decision.action} for '${node_name}'`);
    print(`   Confidence: ${decision.confidence}`);
    print(`   Reason: ${decision.reason}`);
    
    if decision.action == "trace_calls" {
        // Would call codebase-memory trace_path(function_name=node_name, mode="calls")
        print(`   → trace_path("${node_name}", mode="calls", depth=2)`);
    } else if decision.action == "trace_routes" {
        print(`   → trace_path("${node_name}", mode="cross_service", depth=2)`);
    } else if decision.action == "flag" {
        print(`   → flagged for manual review / model classification`);
    }
}

// ── MAIN OODA LOOP ────────────────────────────────────────────────────────
fn ooda_loop() {
    print("🥾 OODA Loop: Knowledge Graph Audit\n");
    
    // PHASE 1: OBSERVE
    print("═══ OBSERVE ═══\n");
    observe_isolates();
    print("");
    observe_degree_distribution();
    print("");
    observe_hubs(MIN_DEGREE_FOR_LAYOUT);
    print("");
    
    // PHASE 2: ORIENT
    print("═══ ORIENT ═══\n");
    orient_clusters();
    
    // Cassowary layout recommendation
    let base_radius = 150.0;
    let max_degree = 50.0;   // will be replaced by actual max from query
    let spread_hub = orient_layout_force(MIN_DEGREE_FOR_LAYOUT, max_degree, base_radius);
    print(`   Layout: hubs spread to ${spread_hub}px (base: ${base_radius})`);
    print("");
    
    // PHASE 3: DECIDE (placeholder — model classification coming)
    print("═══ DECIDE ═══\n");
    print("   Rules-based classifier active (finetuned model training in progress)");
    print("   Sample decision for Function node:");
    let sample_decision = decide_reconnect(
        "example_fn",
        ["Function", "Exported"],
        []
    );
    print(`   → ${sample_decision.action} (confidence: ${sample_decision.confidence})`);
    print("");
    
    // PHASE 4: ACT
    print("═══ ACT ═══\n");
    act_reconnect("example_fn", sample_decision);
    print("");
    
    print("✅ OODA loop complete. Model training will replace decide_reconnect().");
}

// Run the loop
ooda_loop();
