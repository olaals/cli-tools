use tokio::sync::mpsc;
use watchdag::dag::ScheduledTask;
use watchdag::engine::RuntimeEvent;
use watchdag::exec::executor_loop::spawn_executor;

#[tokio::test]
async fn test_rerun_false_long_lived_emits_progress() {
    let (rt_tx, mut rt_rx) = mpsc::channel(10);
    let exec_tx = spawn_executor(rt_tx);

    // 1. Schedule a long-lived task (Run 1)
    let task1 = ScheduledTask {
        name: "B".to_string(),
        cmd: "sleep 100".to_string(),
        long_lived: true,
        rerun: false,
        run_id: 1,
        progress_on_stdout: None,
        trigger_on_stdout: None,
        progress_on_time: None,
        use_hash: false,
    };
    
    exec_tx.send(task1.clone()).await.unwrap();

    // Give it a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 2. Schedule the same task again (Run 2)
    let task2 = ScheduledTask {
        run_id: 2,
        ..task1
    };
    
    exec_tx.send(task2).await.unwrap();

    // 3. Expect TaskProgressed for Run 2 (synthesized by executor)
    // Note: The first task is still running "sleep 100".
    // The executor should see it's running, rerun=false, long_lived=true, and emit TaskProgressed.
    
    let event = tokio::time::timeout(tokio::time::Duration::from_secs(1), rt_rx.recv())
        .await
        .expect("timed out waiting for event")
        .expect("channel closed");

    match event {
        RuntimeEvent::TaskProgressed { task } => {
            assert_eq!(task, "B");
        }
        _ => panic!("expected TaskProgressed, got {:?}", event),
    }
}
