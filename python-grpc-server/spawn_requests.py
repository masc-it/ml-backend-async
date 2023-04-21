import requests
from multiprocessing import Pool
import time
import uuid

def main(a):
    #t0 = time.time()
    res = requests.get("http://localhost:8000")        
    #print(f"{time.time() - t0}")
    return res.text

if __name__ == "__main__":
    
    t0 = time.time()
    with Pool(processes=8) as p:
        results = p.map(main, list(range(10000)))
    
    with open(f"ids_{uuid.uuid4()}.txt", "w") as fp:
        fp.writelines([f"{r}\n" for r in results])
    print(f"elapsed: {time.time() - t0}")