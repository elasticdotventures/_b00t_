// kubernetes_detect.rhai
// Picks a recommended Kubernetes distro for this host and writes _b00t_/kubernetes.<hostname>.toml

fn pick_distro() -> (String, String) {
    // Hard preference order: installed beats available, k0s is the fallback
    if command_exists("k0s") {
        return ("k0s".to_string(), "k0s already installed".to_string());
    }
    if command_exists("k3d") {
        return ("k3d".to_string(), "k3d already installed".to_string());
    }
    if command_exists("minikube") {
        return ("minikube".to_string(), "minikube already installed".to_string());
    }
    if command_exists("kind") {
        return ("kind".to_string(), "kind already installed".to_string());
    }

    // Not installed anywhere; if Docker exists, k3d is a good fit, otherwise default to k0s
    if command_exists("docker") || command_exists("podman") {
        return ("k3d".to_string(), "container runtime detected (docker/podman); choosing k3d".to_string());
    }

    ("k0s".to_string(), "default preference (no distro detected)".to_string())
}

fn target_paths(hostname: &str, workspace_root: &str) -> (String, String) {
    let host_slug = if hostname.is_empty() { "default" } else { hostname };
    let base = if !workspace_root.is_empty() {
        format!("{}/_b00t_", workspace_root)
    } else {
        format!("{}/.b00t", get_env("HOME"))
    };

    let host_file = format!("{}/kubernetes.{}.toml", base, host_slug);
    let shared_file = format!("{}/kubernetes.config.toml", base);
    (host_file, shared_file)
}

fn write_choice(path: &str, distro: &str, reason: &str) {
    let content = format!(
        "[kubernetes]\npreferred = \"{}\"\nreason = \"{}\"\nlast_detected = \"{}\"\n",
        distro,
        reason,
        TIMESTAMP
    );
    let dir = path.split("/").collect::<Vec<_>>();
    if dir.len() > 1 {
        let parent = dir[..dir.len() - 1].join("/");
        create_dir(&parent);
    }
    write_file(path, &content);
}

let (distro, reason) = pick_distro();
let (host_path, shared_path) = target_paths(HOSTNAME, WORKSPACE_ROOT);

log_info(format!("Detected host: {}", if HOSTNAME.is_empty() { "<unknown>" } else { HOSTNAME }));
log_info(format!("Recommended Kubernetes distro: {} ({})", distro, reason));

write_choice(&host_path, &distro, &reason);
write_choice(&shared_path, &distro, &reason);

log_success(format!(
    "Saved detection -> {} and {}",
    host_path,
    shared_path
));
