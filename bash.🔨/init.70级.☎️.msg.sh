#!/bin/bash

## * * * * * * * * * * * \\
#*
#* ☎️ Messaging
#*
## * * * * * * * * * * * //

# ☎️ Messaging

# Celery is a task queue with batteries included.
# https://docs.celeryproject.org/en/stable/getting-started/first-steps-with-celery.html

sudo apt-get install rabbitmq-server

docker run -d -p 5672:5672 rabbitmq
docker run -d -p 6379:6379 redis

pip install celery

# To Run: 
celery -A tasks worker --loglevel=INFO

# Example: 
# from celery import Celery
# app = Celery('tasks', broker='pyamqp://guest@localhost//')
# @app.task
# def add(x, y):
#    return x + y