
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn projection_checkpoint_resumes_monotonically_with_stable_protected_hashes() {
    // Ordered phases preserve shared integration fixtures in lexical scope.
    include!("02_projection_resume_phases/01_canonicalization_and_resume.rs");
}
