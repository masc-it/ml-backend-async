from locust import between, task, FastHttpUser

class HelloWorldUser(FastHttpUser):
    
    @task
    def hello_world(self):
        self.client.get("/")