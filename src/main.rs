#[tokio::main]
async fn main() {
    let task = tokio::spawn(async {
        println!("Hello, world!");
    });

    task.await.unwrap();
}
