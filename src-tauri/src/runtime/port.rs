use std::time::{Duration, Instant};

use crate::async_runtime::JoinHandle;

use crate::platform::platform;

pub fn is_own_process(pid: u32) -> bool {
    pid == std::process::id()
}

pub fn wait_for_port_free_blocking(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match platform().find_pid_listening_on_port(port) {
            Ok(None) => return true,
            Ok(Some(pid)) if is_own_process(pid) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(Some(_)) => return false,
            Err(_) => return false,
        }
    }

    platform()
        .find_pid_listening_on_port(port)
        .ok()
        .flatten()
        .is_none()
}

pub fn await_listener_shutdown_blocking(handle: Option<JoinHandle<()>>, port: u16) {
    if let Some(handle) = handle {
        // begin_stop 已经发送了优雅退出信号。这里必须等待监听端口真正释放，
        // 不能只把等待任务丢到异步运行时后立即返回，否则 restart 会与旧监听器并发启动。
        let port_free = wait_for_port_free_blocking(port, Duration::from_secs(3));
        if !port_free {
            handle.abort();
        }
        crate::async_runtime::spawn(async move {
            let _ = handle.await;
        });
    } else {
        let _ = wait_for_port_free_blocking(port, Duration::from_secs(5));
    }
}

pub fn port_busy_message(port: u16, service_label: &str, pid: u32) -> String {
    let image = platform()
        .process_image_path(pid)
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("pid {pid}"));

    if is_own_process(pid) {
        format!(
            "{service_label}端口 {port} 仍被本应用的上一次服务占用（{image}），请先停止服务或稍后再试"
        )
    } else {
        format!("{service_label}端口 {port} 已被占用：{image}")
    }
}
