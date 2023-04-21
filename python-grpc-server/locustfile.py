from locust import between, task, FastHttpUser

class HelloWorldUser(FastHttpUser):
    
    wait_time = between(1, 5)
    @task
    def hello_world(self):
        self.client.get("/")