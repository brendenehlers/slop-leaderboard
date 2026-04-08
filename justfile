default:
    @just --list

kill-proc:
    kill "$(cat ~/.slop/slop.pid)"

tail-logs:
    tail -f ~/.slop/daemon.out

run:
    cargo run --bin slopmon

up:
    sudo docker compose up --detach

down:
    sudo docker compose down