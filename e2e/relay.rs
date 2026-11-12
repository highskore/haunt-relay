//! End to end against a real haunt core. Copy into haunt's `cli/tests/` and run
//! `cargo test -p haunt-cli --features plugins --test relay` (see README).
#![cfg(feature = "plugins")]

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use haunt_core::plugin_store::{Store, attach};
use haunt_core::plugins::{HostRequest, PluginHost};
use haunt_core::{Core, SurfaceEnv};

const ID: &str = "dev.haunt.relay";

fn repo() -> PathBuf {
    std::env::var_os("HAUNT_PLUGINS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../haunt-plugins"))
        .join("haunt-relay")
}

fn send(sock: &mut UnixStream, env: &SurfaceEnv, nonce: bool, cwd: &str, rest: &str) {
    let nonce = if nonce { format!(r#","nonce":"{}""#, env.nonce) } else { String::new() };
    let line = format!(r#"{{"v":1,"agent":"claude","surface_id":"{}"{nonce},"session_id":"s1","cwd":"{cwd}",{rest}}}"#, env.surface_id);
    sock.write_all(format!("{line}\n").as_bytes()).unwrap();
}

#[test]
fn the_relay_mode_lists_each_agents_last_turn() {
    let dir = repo();
    if !dir.join("haunt-plugin.toml").is_file() {
        eprintln!("skipped: haunt-relay not found; set HAUNT_PLUGINS_DIR");
        return;
    }
    let home = PathBuf::from(format!("/tmp/haunt-fp-relay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "--target", "wasm32-wasip2"])
        .current_dir(&dir)
        .env_remove("CARGO_TARGET_DIR")
        .status()
        .unwrap();
    assert!(status.success());
    let o = Command::new(env!("CARGO_BIN_EXE_haunt")).args(["plugin", "install", dir.to_str().unwrap(), "--yes"]).env("HAUNT_HOME", &home).output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));

    let core = Core::new(&home);
    core.start().unwrap();
    let (api, forged) = (core.register_surface("/tmp").unwrap(), core.register_surface("/tmp").unwrap());
    let set = Store::open(&home).unwrap().load_all(&PluginHost::new().unwrap(), Some(&core.handle())).unwrap();
    assert!(set.errors().is_empty(), "{:?}", set.errors());
    let set = Arc::new(Mutex::new(set));
    attach(&core, &set);
    let modes: Vec<_> = set.lock().unwrap().palette_items(None, "").into_iter().filter(|i| i.is_mode).map(|i| i.id).collect();
    assert_eq!(modes, ["relay"]);

    let mut sock = UnixStream::connect(&api.socket_path).unwrap();
    send(&mut sock, &api, true, "/tmp/api", r#""event":"prompt_submit","query":"Add session refresh""#);
    send(&mut sock, &api, true, "/tmp/api", r#""event":"task_created","task_id":"1","task_subject":"Write tests""#);
    send(&mut sock, &api, true, "/tmp/api", r#""event":"task_updated","task_id":"1","task_status":"completed""#);
    for f in ["a.rs", "b.rs"] {
        send(&mut sock, &api, true, "/tmp/api", &format!(r#""event":"file_edited","tool_name":"Edit","tool_input":{{"file_path":"/tmp/api/src/{f}"}}"#));
    }
    send(&mut sock, &api, true, "/tmp/api", r#""event":"plan_exit","summary":"1. refresh\n2. test""#);
    send(&mut sock, &forged, false, "/tmp/apix", r#""event":"prompt_submit","query":"forged""#);
    std::thread::sleep(Duration::from_millis(400));

    let rows: Vec<_> = set.lock().unwrap().palette_items(Some("relay"), "api").into_iter().map(|i| (i.title, i.subtitle.unwrap_or_default())).collect();
    assert_eq!(
        rows,
        [
            ("api: last turn → brief".to_owned(), "2 files · 1 task".to_owned()),
            ("api: plan only".to_owned(), "2 lines".to_owned()),
            ("api: files touched only".to_owned(), "2 paths".to_owned()),
        ],
        "the forged tab isn't listed"
    );

    let files = set.lock().unwrap().palette_items(Some("relay"), "api").into_iter().find(|i| i.title == "api: files touched only").unwrap().id;
    set.lock().unwrap().take_requests();
    set.lock().unwrap().invoke(ID, &files, true).unwrap();
    let copied: Vec<_> = set.lock().unwrap().take_requests().into_iter().filter_map(|r| match r {
        HostRequest::ClipboardWrite { plugin_id, text, .. } if plugin_id == ID => Some(text),
        _ => None,
    }).collect();
    assert_eq!(copied.len(), 1, "choosing a row copies its brief");
    assert!(copied[0].contains("src/a.rs") && copied[0].contains("src/b.rs"), "{copied:?}");

    let plan = set.lock().unwrap().palette_items(Some("relay"), "api").into_iter().find(|i| i.title == "api: plan only").unwrap().id;
    set.lock().unwrap().invoke(ID, &plan, true).unwrap();
    let copied: Vec<_> = set.lock().unwrap().take_requests().into_iter().filter_map(|r| match r {
        HostRequest::ClipboardWrite { plugin_id, text, .. } if plugin_id == ID => Some(text),
        _ => None,
    }).collect();
    assert!(copied.len() == 1 && copied[0].contains("1. refresh\n2. test"), "the plan keeps its lines: {copied:?}");

    drop(core);
    let _ = std::fs::remove_dir_all(&home);
}
