#!/usr/bin/env python3
"""Read public Actions state for an exact commit when the API is unavailable.
Does not handle credentials or rerun workflows. A completed failure exits 1;
unknown/pending exits 2. Use after pushing and again after doing other work.
"""
import argparse
import json
import re
import time
import urllib.request
from html.parser import HTMLParser
from urllib.parse import quote

class Steps(HTMLParser):
    def __init__(self):
        super().__init__(); self.steps=[]
    def handle_starttag(self,tag,attributes):
        attrs=dict(attributes)
        if tag=='check-step':
            self.steps.append({'name':attrs.get('data-name'),'conclusion':attrs.get('data-conclusion')})

def read(url):
    url += ('&' if '?' in url else '?') + '_suna_refresh=' + str(int(time.time() // 60))
    request=urllib.request.Request(url,headers={'User-Agent':'Suna-actions-status/1.0'})
    with urllib.request.urlopen(request,timeout=20) as response:
        return response.read(4*1024*1024+1).decode('utf-8')

def inspect(repo,sha):
    page=read(f'https://github.com/{repo}/actions?query={quote("branch:main")}')
    blocks=re.split(r'(?=<div[^>]+class="[^"]*Box-row)',page)
    runs=[]
    for block in blocks:
        if not re.search(r'/commit/'+re.escape(sha),block):continue
        match=re.search(r'href="(/[^"]+/actions/runs/\d+)"[^>]*aria-label="([a-z_ ]+):',block)
        if not match:continue
        url='https://github.com'+match.group(1)
        if any(r['url']==url for r in runs):continue
        state=match.group(2).strip().replace(" ","_")
        if state=="currently_running":state="in_progress"
        if state in {'completed','success','successful','passed'} and 'color-fg-success' in block:state='success'
        run={'url':url,'state':state,'failedSteps':[]}
        if run['state']=='failed':
            detail=read(url)
            jobs=sorted(set(re.findall(re.escape(match.group(1))+r'/job/\d+',detail)))
            for job in jobs:
                parser=Steps();parser.feed(read('https://github.com'+job))
                run['failedSteps'].extend(s['name'] for s in parser.steps if s['conclusion']=='failure')
        runs.append(run)
    return runs

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--repo',default='luckysus/Suna');parser.add_argument('--sha',required=True)
    args=parser.parse_args()
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+',args.repo) or not re.fullmatch(r'[a-f0-9]{7,40}',args.sha):parser.error('invalid repository or SHA')
    try:
        runs=inspect(args.repo,args.sha)
    except Exception as error:
        print(json.dumps({'sha':args.sha,'state':'unverified','reason':type(error).__name__}));raise SystemExit(2)
    print(json.dumps({'sha':args.sha,'runs':runs},ensure_ascii=False,indent=2))
    raise SystemExit(1 if any(r['state']=='failed' for r in runs) else 0 if runs and all(r['state']=='success' for r in runs) else 2)
