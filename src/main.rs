fn main(){
    println!("
    🖕 EFS

    1. Setting Up The Project
        1. clone https://github.com/uratne/f___-efs
        2. cd f___-efs
        3. git checkout -b <branch_name>
        4. git submodule update --init --recursive
        5. cargo build
        6. cd frontend
        7. npm install

    2. Running The Project
        1. cargo run --bin server
            i. .env file should be prersent in the current directory
        2. cargo run --bin client
            ii. webtail_config.json file should be present in the current directory
        3. From the frontend directory,
            npm run dev


    3. Building the project
        1. cargo build --release
        2. From the frontend directory,
            npm run build

    4. Build for the static binaries (need to add the target first)
        1. cargo build --target x86_64-unknown-linux-musl --release
        2. cargo build --target x86_64-pc-windows-gnu --release
        3. cargo build --target x86_64-apple-darwin --release
    ");
}