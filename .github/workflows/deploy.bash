#!/usr/bin/env bash

ssh -oStrictHostKeyChecking=accept-new connor@cwfitz.com <<ENDFILE
docker pull cwfitzgerald/file-host:latest
docker stop file-host
docker rm file-host
docker run -d --name file-host --restart always -p 9005:8000 cwfitzgerald/file-host:latest -v file-host:/app/data
docker image prune -f
ENDFILE