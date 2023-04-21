#!/bin/bash

python -m grpc_tools.protoc --proto_path=../protos ../protos/prediction.proto --python_out=. --grpc_python_out=.