use std::convert::Infallible;

use anyhow::anyhow;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use systemd::daemon;
use tokio::signal::unix::{signal, SignalKind};

async fn hello_world(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new("Hello, World!!\n".into()))
}

async fn shutdown_signal() {
    let mut interrupt = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    let mut terminate = signal(SignalKind::terminate()).expect("instlal SIGTERM handler");

    tokio::select! {
        _ = interrupt.recv() => {
            println!("SIGINT received");
        },
        _ = terminate.recv() => {
            println!("SIGTERM received");
        },
    }

    if daemon::notify(false, [(daemon::STATE_STOPPING, "1")].iter())
        .expect("failed to notify stopping to systemd")
    {
        println!("sent STATE_STOPPING to systmd");
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let fds = daemon::listen_fds(false)?;
    if fds.len() != 1 {
        return Err(anyhow!(
            "want 1 activated socket, but got {} socket(s)",
            fds.len()
        ));
    }
    let std_listener = daemon::tcp_listener(fds.iter().next().unwrap())?;
    std_listener.set_nonblocking(true)?;

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(hello_world)) });
    let server = Server::from_tcp(std_listener)?.serve(make_svc);
    let graceful = server.with_graceful_shutdown(shutdown_signal());
    if daemon::notify(false, [(daemon::STATE_READY, "1")].iter())? {
        println!("sent STATE_READY to systmd");
    }
    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
    Ok(())
}
