use tokio::sync::{mpsc, oneshot};

#[tokio::main]
async fn main2() {
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let a = rx.await.unwrap();
    });
    tx.send(1);
}

fn needs_send<T: Send>(t: T) {}

async fn bar() {}

async fn foo() {
    let m = std::sync::Mutex::new(1);
    let mut guard = m.lock().unwrap();
    bar().await;
    *guard = 2;
}

#[tokio::test]
async fn test() {
    needs_send(foo())
}
