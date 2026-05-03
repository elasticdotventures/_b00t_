#!/bin/bash

#	--data '{
#	"model": "fireworks-llama-v3-70b-instruct",


curl --location 'http://0.0.0.0:4000/chat/completions' \
	--header 'Content-Type: application/json' \
	--header 'Authorization: Bearer sk-1234' \
	--data '{
	"model": "fireworks-deepseek-v3",
	"messages": [
		{
		"role": "user",
		"content": "what llm are you"
		}
	]
}'

curl --location 'http://0.0.0.0:4000/chat/completions' \
	--header 'Content-Type: application/json' \
	--header 'Authorization: Bearer sk-1234' \
	--data '{
	"model": "fireworks-deepseek-v3",
	"messages": [
		{
		"role": "user",
		"content": "what llm are you"
		}
	]
}'
