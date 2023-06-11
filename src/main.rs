use std::{collections::HashMap, convert::Infallible, time::Duration};

use anyhow::anyhow;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use systemd::daemon;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::oneshot,
    time::sleep,
};
use url::form_urlencoded;

static INVALID_WAIT: &[u8] = b"\"wait\" query parameter must be unsigned integer for seconds";

async fn hello_world(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    if let Some(q) = req.uri().query() {
        let params = form_urlencoded::parse(q.as_bytes())
            .into_owned()
            .collect::<HashMap<String, String>>();
        if let Some(wait) = params.get("wait") {
            match wait.parse::<u64>() {
                Ok(seconds) => sleep(Duration::from_secs(seconds)).await,
                Err(_) => {
                    return Ok(Response::builder()
                        .status(StatusCode::UNPROCESSABLE_ENTITY)
                        .body(INVALID_WAIT.into())
                        .unwrap())
                }
            }
        }
    }
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

    let (tx, rx) = oneshot::channel();
    let (tx2, rx2) = oneshot::channel();
    tokio::spawn(async move {
        shutdown_signal().await;
        tx.send(()).unwrap();
        sleep(Duration::from_secs(3)).await;
        tx2.send(()).unwrap();
    });
    let graceful = server.with_graceful_shutdown(async {
        rx.await.ok();
    });
    if daemon::notify(false, [(daemon::STATE_READY, "1")].iter())? {
        println!("sent STATE_READY to systmd");
    }
    tokio::select! {
        _ = rx2 => {
            println!("force shutdown");
        },
        res = graceful => {
            if let Err(e) = res {
                eprintln!("server error: {}", e);
            } else {
                println!("finished graceful shutdown");
            }
        }
    }
    Ok(())
}
