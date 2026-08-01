#[test]
fn kill9_at_claim_provider_tool_and_event_boundaries_is_exactly_once() {
    let temporary = temporary_root();
    for boundary in BOUNDARIES {
        let scenario_root = temporary.0.join(boundary);
        fs::create_dir(&scenario_root).expect("kill9 scenario root must be created");
        let repository = FileAgentJobRepository::new(&scenario_root);
        repository.initialize(&format!("job_ar09_kill9_{boundary}"));

        let mut child = spawn_child(&scenario_root, boundary, FIRST_RUN_NOW);
        let stdout = child
            .stdout
            .take()
            .expect("kill9 child stdout must be captured");
        let (sender, receiver) = mpsc::channel();
        let expected_marker = format!("{READY_PREFIX}{boundary}");
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.expect("kill9 child output must be readable");
                if line.contains(&expected_marker) {
                    sender
                        .send(expected_marker.clone())
                        .expect("kill9 readiness receiver must remain alive");
                    break;
                }
            }
        });
        let marker = receiver
            .recv_timeout(Duration::from_secs(15))
            .unwrap_or_else(|_| panic!("child did not reach kill9 boundary {boundary}"));
        assert_eq!(marker, format!("{READY_PREFIX}{boundary}"));
        child.kill().expect("kill9 child must accept SIGKILL");
        let status = child.wait().expect("kill9 child must be reaped");
        reader.join().expect("kill9 output reader must finish");
        assert_eq!(
            status.signal(),
            Some(9),
            "{boundary} must terminate through SIGKILL"
        );

        let recovery = Command::new(
            env::current_exe().expect("current test executable must be available for recovery"),
        )
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("AR09_KILL9_ROOT", &scenario_root)
        .env("AR09_KILL9_BOUNDARY", "none")
        .env("AR09_KILL9_NOW", RECOVERY_NOW.to_string())
        .output()
        .expect("kill9 recovery child must start");
        assert!(
            recovery.status.success(),
            "recovery failed at {boundary}: stdout={} stderr={}",
            String::from_utf8_lossy(&recovery.stdout),
            String::from_utf8_lossy(&recovery.stderr),
        );

        let state = repository.load_state();
        assert_eq!(state.state, WorkflowState::Completed.as_str(), "{boundary}");
        assert_eq!(state.linked_event_sequences, vec![9_001], "{boundary}");
        assert_eq!(state.decision_json, None, "{boundary}");
        assert_eq!(state.tool_result_json, None, "{boundary}");
        assert!(
            state
                .evidence
                .keys()
                .any(|key| key.ends_with(":canonical_commit")),
            "{boundary} must retain canonical evidence"
        );
        assert!(
            state.evidence.keys().any(|key| key.ends_with(":completed")),
            "{boundary} must retain completion evidence"
        );
        assert!(
            scenario_root.join("tool-receipt").is_file(),
            "{boundary} must have one idempotent tool receipt"
        );
        assert!(
            scenario_root.join("canonical-event-receipt").is_file(),
            "{boundary} must have one idempotent canonical receipt"
        );
        let canonical_receipts = fs::read_dir(&scenario_root)
            .expect("kill9 scenario root must remain readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name() == "canonical-event-receipt")
            .count();
        assert_eq!(
            canonical_receipts, 1,
            "{boundary} must produce at most one canonical event receipt"
        );
        println!(
            "AR09_KILL9_VERIFIED boundary={boundary} signal=9 canonical_event_receipts={canonical_receipts}"
        );
    }
}
