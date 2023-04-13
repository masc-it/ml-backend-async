# Async ML backend

## Run

    cargo run

# Docker

Note: The first docker build will be pretty slow.

    docker build -t mascit/ml-backend-async .

    docker run -p 8000:8000 --rm  mascit/ml-backend-async