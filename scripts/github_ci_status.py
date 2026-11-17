"""Read public branch CI status without reading or printing credentials."""
import json
import sys
import urllib.request

url = ('https://api.github.com/repos/GodWar9/lstm_stock_prediction/actions/runs/'
       + sys.argv[1])
with urllib.request.urlopen(url, timeout=30) as response:
    run = json.load(response)
print(json.dumps({key: run.get(key) for key in
                 ('id', 'status', 'conclusion', 'head_sha', 'html_url', 'updated_at')}, indent=2))
