up:
	cargo build
	docker build . -t rusty-blockchain -f ./test/Dockerfile
	docker compose -f ./test/docker-compose.yml up -d

down:
	docker compose -f ./test/docker-compose.yml down

