# Asynchronouns Rust
## Introduction
Asynchronous code is everywhere now. It's basically a must when you write anything that needs to scale, like API/backend applications. There are different ways to tackle async code in Rust, but we will use the most popular crate for it, called Tokio. Ultimately, we will create a very simple API that can handle multiple requests at a time. Let's start!

The final code can be found here: [Tokio tutorial Github repo](https://github.com/jbarszczewski/rust-tokio-tutorial).

## Simple async hello-rust
Let's start by creating a basic program that executes a task. Create a project:
```shell
$ cargo new tokio-tutorial
$ cd tokio-tutorial
```

For this task we will need just one dependency. So open `Cargo.toml` and add it:
```toml
[dependencies]
tokio = {version = "1.14.0", features = ["full"]}
```
Now got to `src/main.rs` and replace content with:
```rust
#[tokio::main]
async fn main() {
    let task = tokio::spawn(async {
        println!("Hello, rust!");
    });

    task.await.unwrap();
}
```

And that's all you need to run (`cargo run`) a simple async hello-rust task!
The full code for this chapter can be found [on my Github](https://github.com/jbarszczewski/rust-tokio-tutorial/tree/d31121d512c092ad82440be01a1eeecb118fecde).
Of course, this example doesn't show the real power of Tokio runtime so let's jump on to more useful example.

## Savings Balance API
Ok, to spare you from creating another ToDo list API we will do something even simpler: Savings Balance API. The aim is simple, we will expose two methods: GET and POST, to manage our balance. GET will return the current value and POST will add/substract from it. If you went through the Rust Book you probably stumbled across the [Multithreaded Web Server project](https://doc.rust-lang.org/stable/book/ch20-00-final-project-a-web-server.html). It's a really great starting point to get your head around threads but requires a lot of boilerplate code (manual thread management etc.). This is where Tokio comes in.
### First response
We will start with a simple server that listens for incoming requests. Replace the content of `main.rs` with:
```rust
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8181").await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        handle_connection(stream).await;
    }
}

async fn handle_connection(mut stream: TcpStream) {
    let contents = "{\"balance\": 0.00}";

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        contents.len(),
        contents
    );
    stream.write(response.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}
```

Run it and in your browser navigate to [http://127.0.0.1:8181](http://127.0.0.1:8181) to see your first response from the server. Some code explanaition:
- TCP listener is created and bound to our local address.
- In a loop we await for an incoming connection.
- Once connection is made, we pass the stream to our handler

*Ok but it's not multitasking!* 
Exacly, our code is processing only one request at a time. So how do we make it process connections concurrently? Very simple. Just wrap the `handle_connection()` in a `tokio::spawn` function:
```rust
tokio::spawn(async move {
    handle_connection(stream).await;
});
```
And that's it! You now can process multiple connections at a time!
Code so far [Can be found on GitHub here](https://github.com/jbarszczewski/rust-tokio-tutorial/tree/12cad3ba7b7528030de26f9678f514ccdbaf4b68)

### GET and POST
Before we move to the last part of the tutorial: modyfing balance value, we need to make sure we can read and change the balance.

To keep things simple, we will have two scenarion:
- GET http://127.0.0.1:8181 
   As soon as we detect GET request, we return balance. 
- POST http://127.0.0.1:8181/62.32
    If the method is POST we will read value (max 10 characters) from route, update our balance and return it.

Of course, this is not the most RESTful or scalable approach but will work for this tutorial just fine.

The new `handle_connection` is looking like this:
```rust
async fn handle_connection(mut stream: TcpStream) {
    // Read the first 16 characters from the incoming stream.
    let mut buffer = [0; 16];
    stream.read(&mut buffer).await.unwrap();
    // First 4 characters are used to detect HTTP method
    let method_type = match str::from_utf8(&buffer[0..4]) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };
    
    let contents = match method_type {
        "GET " => {
            // todo: return real balance
            format!("{{\"balance\": {}}}", 0.0)
        }
        "POST" => {
            // Take characters after 'POST /' until whitespace is detected.
            let input: String = buffer[6..16]
                .iter()
                .take_while(|x| **x != 32u8)
                .map(|x| *x as char)
                .collect();
            let balance_update = input.parse::<f32>().unwrap();
            // todo: add balance update handling
            format!("{{\"balance\": {}}}", balance_update)
        }
        _ => {
            panic!("Invalid HTTP method!")
        }
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        contents.len(),
        contents
    );
    stream.write(response.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}
```
The idea is that we read first n characters from the incoming request and use that to perform selected operation. For the demo purpouse and simplicity we limit our input to a maximum of 10 characters.
Try running it now and see how response changes depending on chosen method. Example cURL command:
```curl
curl --request POST 'http://127.0.0.1:8181/-12.98'
```

### Handling The Balance
So far, our handler returns hardcoded results. Let's introduce a variable that will keep the value inbetween the calls. Our balance variable will be shared across tasks and potentially threads so to support this it is wrapped in `Arc<Mutex<_>>`. `Arc<>` allows variable to be referenced concurrently from many tasks, and `Mutex<>` is a guard that make sure only one tasks can change it at a time (other tasks will have to wait for their turn).

Below is the full code with balance update. Check the comments on new lines:
```rust
use std::str;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    // create balance wrapped in Arc and Mutex for cross thread safety
    let balance = Arc::new(Mutex::new(0.00f32));
    let listener = TcpListener::bind("127.0.0.1:8181").await.unwrap();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        // Clone the balance Arc and pass it to handler
        let balance = balance.clone();
        tokio::spawn(async move {
            handle_connection(stream, balance).await;
        });
    }
}

async fn handle_connection(mut stream: TcpStream, balance: Arc<Mutex<f32>>) {
    // Read the first 16 characters from the incoming stream.
    let mut buffer = [0; 16];
    stream.read(&mut buffer).await.unwrap();
    // First 4 characters are used to detect HTTP method
    let method_type = match str::from_utf8(&buffer[0..4]) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };
    let contents = match method_type {
        "GET " => {
            // before using balance we need to lock it.
            format!("{{\"balance\": {}}}", balance.lock().unwrap())
        }
        "POST" => {
            // Take characters after 'POST /' until whitespace is detected.
            let input: String = buffer[6..16]
                .iter()
                .take_while(|x| **x != 32u8)
                .map(|x| *x as char)
                .collect();
            let balance_update = input.parse::<f32>().unwrap();

            // acquire lock on our balance and update the value
            let mut locked_balance: MutexGuard<f32> = balance.lock().unwrap();
            *locked_balance += balance_update;
            format!("{{\"balance\": {}}}", locked_balance)
        }
        _ => {
            panic!("Invalid HTTP method!")
        }
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        contents.len(),
        contents
    );
    stream.write(response.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}
```
Please note that we are acquiring lock on our balance, but we don't need to unlock it manually. Once the `MutexGuard` returned by `lock()` goes out of scope, the lock is automatically removed. In our situation it is straight after we're done with preparing response contents. 

Now we are ready to run our app and test if we are getting a proper balance response. Example cURL command:
```curl
curl --request POST 'http://127.0.0.1:8181/150.50'
```

Here is an exercise for you: To see how using lock affects execution of the code try adding some delay (take a look at `tokio::timer::Delay`), long enough to make multiple API calls, just after acquiring a lock!

## Conclusion
As we can see it's really easy to create an application in Rust that handle async operations thans to Tokio runtime. Of course, for API usually you will use a web framework like Actix, Rocket or Warp (among others) but hopefully you will have a better understanding of how it works "under the hood".

The final code can be found here: [Tokio tutorial Github repo](https://github.com/jbarszczewski/rust-tokio-tutorial).

I hope you've enjoyed this tutorial and as always if you have any suggestions/questions don't hesitate to leave a comment below.

Thanks for reading and till the next time!