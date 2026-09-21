#!/usr/bin/env python3
"""Fresh, non-executing parser for declarative Torque mission object blocks.

Keeps raw source coordinates; does not guess transforms or decode DIF/DTS/TER.
Unsupported syntax fails explicitly instead of yielding a partial map.
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re

TOKEN = re.compile(r'\s+|//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"|'
                   r'[A-Za-z_][A-Za-z_0-9:]*|[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?|[{}()\[\]=;,]')
IDENT = re.compile(r'[A-Za-z_][A-Za-z_0-9:]*\Z')
NUMBER = re.compile(r'[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?\Z')
DEPENDENCIES = {'terrainfile':'terrain', 'interiorfile':'interior', 'shapename':'shape',
                'materiallist':'sky-material-list', 'graphfile':'navigation', 'filename':'file'}


class MissionError(ValueError): pass


def tokens(text):
    pos=0; result=[]
    while pos<len(text):
        match=TOKEN.match(text,pos)
        if not match:
            raise MissionError(f'Unsupported token at line {text.count(chr(10),0,pos)+1}: {text[pos:pos+24]!r}')
        value=match.group();pos=match.end()
        if not value.isspace() and not value.startswith(('//','/*')): result.append(value)
    return result


def scalar(value):
    if value.startswith('"'):
        # Torque strings are not JSON: preserve unknown escapes as literal data.
        return re.sub(r'\\([\\"nrt])',lambda m:{'n':'\n','r':'\r','t':'\t','"':'"','\\':'\\'}[m[1]],value[1:-1])
    if IDENT.fullmatch(value) or NUMBER.fullmatch(value): return value
    raise MissionError('Expected literal value, got '+value)


def parse(text):
    begin='//--- OBJECT WRITE BEGIN ---';end='//--- OBJECT WRITE END ---'
    outside=False
    if begin in text:
        before,body=text.split(begin,1)
        if end not in body: raise MissionError('Missing OBJECT WRITE END marker')
        body,after=body.split(end,1)
        outside=bool(re.sub(r'//[^\n]*|/\*[\s\S]*?\*/|\s+','',before+after))
    else: body=text
    stream=tokens(body); cursor=0; objects=[]; warnings=[]
    def take(expected=None):
        nonlocal cursor
        if cursor>=len(stream): raise MissionError('Unexpected end of object block')
        value=stream[cursor];cursor+=1
        if expected is not None and value!=expected: raise MissionError(f'Expected {expected!r}, got {value!r}')
        return value
    def peek(): return stream[cursor] if cursor<len(stream) else None
    def identifier():
        value=take()
        if not IDENT.fullmatch(value): raise MissionError('Expected identifier: '+value)
        return value
    def obj(parent,depth):
        if depth>64 or len(objects)>=100000: raise MissionError('Object nesting/count limit exceeded')
        take('new');kind=identifier();take('(')
        name='' if peek()==')' else scalar(take())
        take(')');take('{')
        record={'id':len(objects),'parent':parent,'kind':kind,'name':name,'fields':{}}
        objects.append(record)
        while peek()!='}':
            if peek()=='new': obj(record['id'],depth+1);continue
            field=identifier()
            if peek()=='[':
                take('[');index=take();take(']')
                if not index.isdigit(): raise MissionError('Only numeric array indices supported')
                field+='['+index+']'
            take('=');value=scalar(take());take(';')
            if field in record['fields']: raise MissionError('Duplicate field: '+field)
            record['fields'][field]=value
        take('}');take(';')
    while peek() is not None: obj(None,0)
    if outside: warnings.append('Executable content outside object block was NOT executed or translated')
    return {'schema':1,'coordinate_space':'raw Torque source coordinates; untransformed',
            'objects':objects,'warnings':warnings}


def inventory(root,output):
    if output.exists(): raise ValueError('Refusing to overwrite mission collection')
    files=[p for p in root.rglob('*') if p.is_file()]
    index=defaultdict(list)
    for path in files: index[path.name.lower()].append(path)
    results=[];output.mkdir(parents=True)
    for path in sorted(p for p in files if p.suffix.lower()=='.mis'):
        relative=str(path.relative_to(root));raw=path.read_bytes()
        row={'mission':relative,'sha256':hashlib.sha256(raw).hexdigest()}
        try:
            document=parse(raw.decode('utf-8',errors='strict'))
            deps=[]
            for obj in document['objects']:
                for field,value in obj['fields'].items():
                    if field.lower() not in DEPENDENCIES: continue
                    name=value.replace('\\','/').split('/')[-1]
                    candidates=index.get(name.lower(),[])
                    deps.append({'object':obj['id'],'field':field,'reference':value,
                        'candidates':[str(p.relative_to(root)) for p in candidates],
                        'status':'missing' if not candidates else 'found' if len(candidates)==1 else 'ambiguous'})
            document['source']=row.copy();document['dependencies']=deps
            kinds=Counter(o['kind'] for o in document['objects'])
            blocks=Counter(o['fields'].get('dataBlock','') for o in document['objects'])
            row.update(status='parsed-not-converted',objects=len(document['objects']),kinds=dict(kinds),
                dependency_status=dict(Counter(d['status'] for d in deps)),
                flags=blocks.get('FLAG',0),warnings=document['warnings'])
            destination=output/Path(relative).with_suffix('.json');destination.parent.mkdir(parents=True,exist_ok=True)
            destination.write_text(json.dumps(document,indent=2)+'\n')
        except (MissionError,UnicodeError) as error:
            row.update(status='unsupported',error=str(error))
        results.append(row)
    (output/'index.json').write_text(json.dumps({'schema':1,'missions':results},indent=2)+'\n')
    print(json.dumps(dict(Counter(row['status'] for row in results))))
    for row in results:
        if 'Stonehenge_nef' in row['mission']: print(json.dumps(row,indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args()
    private=(Path(__file__).resolve().parents[2]/'local-assets').resolve()
    if not args.output.resolve().is_relative_to(private): parser.error('Use ignored local-assets for private mission output')
    inventory(args.source,args.output)
