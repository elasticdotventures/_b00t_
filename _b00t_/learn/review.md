---
bouncer-review: Always check every code path that passes the same parameter — handle_scan got tilde expansion but handle_quiz didn't, even though both receive b00t_path from the same source.
