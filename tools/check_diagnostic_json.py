#!/usr/bin/env python3
"""Validate schema 1 diagnostic JSON Lines from stdin; standard library only."""
import json
import sys
KEYS = {'schema_version', 'severity', 'code', 'message', 'spans', 'notes', 'helps', 'suggestions', 'claim', 'claim_after_computing', 'facts_considered', 'counterexample', 'suggested_explicit_form'}
LOCATION = {'file','byte_start','byte_end','line_start','column_start','line_end','column_end'}
def location(span):
    assert isinstance(span['file'],str)
    for key in ('byte_start','byte_end'):
        assert type(span[key]) is int and span[key]>=0
    assert span['byte_end']>=span['byte_start']
    for key in ('line_start','column_start','line_end','column_end'):
        assert span[key] is None or (type(span[key]) is int and span[key]>=1)
def validate(line):
    array=json.loads(line)
    assert isinstance(array,list) and len(array)==1
    d=array[0]
    assert set(d)==KEYS, set(d)^KEYS
    assert d['schema_version']==1 and d['severity'] in ('error','warning')
    assert isinstance(d['code'],str) and len(d['code'])==5 and d['code'][0]=='L' and d['code'][1:].isdigit()
    assert isinstance(d['message'],str)
    assert sum(span['primary'] for span in d['spans'])==1
    for span in d['spans']:
        assert set(span)==LOCATION|{'primary','label'}
        assert type(span['primary']) is bool and isinstance(span['label'],str)
        location(span)
    for key in ('notes','helps'):
        assert isinstance(d[key],list) and all(isinstance(v,str) for v in d[key])
    for suggestion in d['suggestions']:
        assert set(suggestion)=={'message','span','replacement','applicability'}
        assert set(suggestion['span'])==LOCATION
        assert isinstance(suggestion['message'],str) and isinstance(suggestion['replacement'],str)
        assert suggestion['applicability'] in ('machine_applicable','maybe_incorrect')
        location(suggestion['span'])
    for key in ('claim','claim_after_computing','counterexample','suggested_explicit_form'):
        assert d[key] is None or isinstance(d[key],str)
    if d['facts_considered'] is not None:
        assert isinstance(d['facts_considered'],list)
        for fact in d['facts_considered']:
            assert set(fact)=={'name','claim'}
            assert fact['name'] is None or isinstance(fact['name'],str)
            assert isinstance(fact['claim'],str)
for number,line in enumerate(sys.stdin,1):
    try: validate(line)
    except Exception as error: raise AssertionError(f'line {number}: {error}\n{line}') from error
