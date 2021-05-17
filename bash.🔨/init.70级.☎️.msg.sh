#!/bin/bash

## * * * * * * * * * * * \\
#*
#* ☎️ Messaging
#*
## * * * * * * * * * * * //

# ☎️ Messaging

# Celery is a task queue with batteries included.
# https://docs.celeryproject.org/en/stable/getting-started/first-steps-with-celery.html
# TODO: Flower is monitoring toolkit (for celery)
# https://flower.readthedocs.io/en/latest/ 

## 🐇 rabbiqmq
# sudo apt-get install rabbitmq-server
# rpi rabbitmq https://hub.docker.com/r/arm32v7/rabbitmq
# docker run -d -p 5672:5672 rabbitmq

## ☎️ redis
docker run -d -p 6379:6379 redis

## celeary
pip install celery

# To Run: 
celery -A tasks worker --loglevel=INFO

# Example: 
# from celery import Celery
# app = Celery('tasks', broker='pyamqp://guest@localhost//')
# @app.task
# def add(x, y):
#    return x + y