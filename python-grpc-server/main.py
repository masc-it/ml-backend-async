import grpc
from concurrent import futures
import prediction_pb2_grpc as prediction_grpc
import prediction_pb2 as prediction
import random
import time
import json
from transformers import pipeline
import os
from dotenv import load_dotenv
from copy import deepcopy
load_dotenv()

class Model:
    def __init__(self, model_path: str) -> None:
        self.token_classifier = pipeline(
            "token-classification", 
            model=model_path, 
            aggregation_strategy="max",
            batch_size=1024
        )
        print("loaded model")
        
    def predict(self, batch):
        preds = self.token_classifier(batch)
        #print(preds)
        preds = self.post_process(preds)
        #print(preds)
        return preds
    
    def post_process(self, preds):
        
        result = []
        
        for output in preds: # iterate through preds
            res = {}
            for group in output:
                
                res[group["entity_group"]] = {
                    "text": group["word"],
                    "score": float(group["score"])
                }
            result.append(res)
        return result


class PredictionService(prediction_grpc.PredictionServicer):

    def __init__(self, *args, **kwargs):
        self.model = Model(os.environ["MODEL_PATH"])
        
    def Predict(self, request, context):

        # get the string from the incoming request
        #messages = request.input
        #print(messages)
        batch = list(request.input)
        
        #print(len(batch))
        preds = self.model.predict(batch) # this consumes the iterator
        result = {'prediction': [json.dumps(pred) for pred in preds], "uuid": request.uuid}
        return prediction.PredictionResponse(**result)

def serve():
    
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=8))
    prediction_grpc.add_PredictionServicer_to_server(PredictionService(), server)
    server.add_insecure_port('[::1]:8080')
    server.start()
    server.wait_for_termination()


if __name__ == '__main__':
    serve()