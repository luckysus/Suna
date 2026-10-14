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
        data=response.read(4*1024*1024+1)
        if len(data)>4*1024*1024:raise ValueError('Actions page exceeds observation budget')
        return data.decode('utf-8')

class EmbeddedData(HTMLParser):
    def __init__(self):
        super().__init__(); self.active=False; self.parts=[]; self.values=[]
    def handle_starttag(self,tag,attributes):
        if tag=='script':
            self.active=dict(attributes).get('type')=='application/json'; self.parts=[]
    def handle_data(self,data):
        if self.active:self.parts.append(data)
    def handle_endtag(self,tag):
        if tag=='script' and self.active:
            try:self.values.append(json.loads(''.join(self.parts)))
            except (ValueError,TypeError):pass
            self.active=False

def embedded_runs(page,repo,sha):
    parser=EmbeddedData();parser.feed(page);rows=[]
    def walk(value):
        if isinstance(value,dict):
            candidates=value.get('workflowRuns')
            if isinstance(candidates,list):rows.extend(candidates)
            for child in value.values():walk(child)
        elif isinstance(value,list):
            for child in value:walk(child)
    for value in parser.values:walk(value)
    runs=[];seen=set()
    for row in rows:
        if not isinstance(row,dict):continue
        description=row.get('description')
        if not isinstance(description,dict):continue
        commit=description.get('linkUrl','')
        if not isinstance(commit,str):continue
        match=re.fullmatch('/'+re.escape(repo)+r'/commit/([a-f0-9]{40})',commit)
        url=row.get('runUrl','')
        if not match or not match.group(1).startswith(sha) or not isinstance(url,str) or not re.fullmatch('https://github.com/'+re.escape(repo)+r'/actions/runs/\d+',url) or url in seen:continue
        state=row.get('state')
        if not isinstance(state,dict):state={}
        status=state.get('status','');conclusion=state.get('conclusion','')
        if status=='WORKFLOW_RUN_STATUS_COMPLETED':
            result={'WORKFLOW_RUN_CONCLUSION_SUCCESS':'success','WORKFLOW_RUN_CONCLUSION_FAILURE':'failed',
                    'WORKFLOW_RUN_CONCLUSION_TIMED_OUT':'failed','WORKFLOW_RUN_CONCLUSION_CANCELLED':'cancelled'}.get(conclusion,'unverified')
        else:
            result={'WORKFLOW_RUN_STATUS_IN_PROGRESS':'in_progress','WORKFLOW_RUN_STATUS_QUEUED':'pending',
                    'WORKFLOW_RUN_STATUS_REQUESTED':'pending','WORKFLOW_RUN_STATUS_WAITING':'pending'}.get(status,'unverified')
        seen.add(url);runs.append({'url':url,'state':result,'failedSteps':[]})
    return runs

def complete_job_evidence(run):
    """Require every job and every visible step, including job completion."""
    relative=run['url'].removeprefix('https://github.com')
    page=read(run['url'])
    jobs=[]
    for path in sorted(set(re.findall(re.escape(relative)+r'/job/\d+',page))):
        parser=Steps();parser.feed(read('https://github.com'+path))
        jobs.append({'url':'https://github.com'+path,'steps':parser.steps})
    verified=bool(run['state']=='success' and jobs and all(
        job['steps'] and any(s['name']=='Complete job' for s in job['steps'])
        and all(s['conclusion']=='success' for s in job['steps']) for job in jobs))
    return {'jobs':jobs,'allJobStepsSuccess':verified}

def inspect(repo,sha):
    page=read(f'https://github.com/{repo}/actions?query={quote("branch:main")}')
    current=embedded_runs(page,repo,sha)
    if current:
        for run in current:
            if run['state']=='failed':
                detail=read(run['url'])
                jobs=sorted(set(re.findall(re.escape(run['url'].removeprefix('https://github.com'))+r'/job/\d+',detail)))
                for job in jobs:
                    parser=Steps();parser.feed(read('https://github.com'+job))
                    run['failedSteps'].extend(s['name'] for s in parser.steps if s['conclusion']=='failure')
        return current
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
        if state in {'completed','success','successful','passed','completed_successfully'} and 'color-fg-success' in block:state='success'
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
        evidence=[complete_job_evidence(run) for run in runs if run['state']=='success']
        complete=bool(runs and len(evidence)==len(runs) and all(item['allJobStepsSuccess'] for item in evidence))
    except Exception as error:
        print(json.dumps({'sha':args.sha,'state':'unverified','reason':type(error).__name__}));raise SystemExit(2)
    print(json.dumps({'sha':args.sha,'runs':runs,'jobEvidence':evidence,'allJobStepsSuccess':complete},ensure_ascii=False,indent=2))
    raise SystemExit(1 if any(r['state']=='failed' for r in runs) else 0 if complete else 2)
