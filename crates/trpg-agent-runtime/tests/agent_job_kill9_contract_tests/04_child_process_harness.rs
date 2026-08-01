fn write_once(path: &Path, contents: &[u8]) {
    match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(mut file) => {
            file.write_all(contents)
                .expect("durable receipt must be written");
            file.sync_all().expect("durable receipt must be synced");
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            assert_eq!(
                fs::read(path).expect("existing durable receipt must be readable"),
                contents
            );
        }
        Err(error) => panic!("durable receipt must be created: {error}"),
    }
}

fn parse_state(value: &str) -> WorkflowState {
    match value {
        "REQUESTED" => WorkflowState::Requested,
        "CLAIMED" => WorkflowState::Claimed,
        "AGENT_RUNNING" => WorkflowState::AgentRunning,
        "AWAITING_TOOL" => WorkflowState::AwaitingTool,
        "COMMITTING" => WorkflowState::Committing,
        "COMPLETED" => WorkflowState::Completed,
        "RETRYABLE_FAILED" => WorkflowState::RetryableFailed,
        "TERMINAL_FAILED" => WorkflowState::TerminalFailed,
        other => panic!("unexpected kill9 workflow state: {other}"),
    }
}

fn crash_boundary(expected: &str) {
    if env::var("AR09_KILL9_BOUNDARY").as_deref() != Ok(expected) {
        return;
    }
    println!("{READY_PREFIX}{expected}");
    std::io::stdout()
        .flush()
        .expect("kill9 readiness marker must flush");
    loop {
        thread::park_timeout(Duration::from_secs(60));
    }
}

fn worker(root: &Path) -> AgentJobWorker {
    AgentJobWorker::new(
        Arc::new(FileAgentJobRepository::new(root)),
        Arc::new(Kill9Provider::new()),
        Arc::new(DurableToolPort {
            root: root.to_owned(),
        }),
        Arc::new(DurableDecisionPort {
            root: root.to_owned(),
        }),
        None,
        AgentJobExecutionConfig {
            claim_owner: "agent_worker_ar09_kill9".to_owned(),
            lease_duration: Duration::from_millis(100),
            heartbeat_interval: Duration::from_millis(10),
            max_attempts: 5,
            max_context_bytes: 64 * 1024,
            max_input_tokens: 1_000,
            max_output_tokens: 1_000,
            max_tool_calls: 1,
            max_tool_loops: 1,
        },
    )
    .expect("kill9 worker configuration must be valid")
}

#[tokio::test]
async fn agent_job_kill9_child() {
    let (root, now_unix_ms, _temporary_root) = match env::var("AR09_KILL9_ROOT") {
        Ok(root) => {
            let now_unix_ms = env::var("AR09_KILL9_NOW")
                .expect("kill9 child now is required")
                .parse()
                .expect("kill9 child now must be numeric");
            (PathBuf::from(root), now_unix_ms, None)
        }
        Err(env::VarError::NotPresent) => {
            let temporary = temporary_root();
            FileAgentJobRepository::new(&temporary.0).initialize("job_ar09_kill9_standalone_child");
            (temporary.0.clone(), FIRST_RUN_NOW, Some(temporary))
        }
        Err(env::VarError::NotUnicode(_)) => {
            panic!("kill9 child root must be valid Unicode")
        }
    };
    let outcome = worker(&root)
        .run_once(now_unix_ms)
        .await
        .expect("kill9 recovery worker must execute");
    assert!(matches!(outcome, AgentJobOutcome::Completed { .. }));
}

struct TemporaryRoot(PathBuf);

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temporary_root() -> TemporaryRoot {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "trpg-ar09-kill9-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("unique kill9 root must be created");
    TemporaryRoot(root)
}

fn spawn_child(root: &Path, boundary: &str, now_unix_ms: i64) -> std::process::Child {
    Command::new(env::current_exe().expect("current test executable must be available"))
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_KILL9_ROOT", root)
        .env("AR09_KILL9_BOUNDARY", boundary)
        .env("AR09_KILL9_NOW", now_unix_ms.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("kill9 child must start")
}
