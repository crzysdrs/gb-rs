SHELL := /bin/bash

all:
	ln -s .pre-commit .git/.pre-commit

tmpdiff.log : gold.log run.log
	sdiff <(cat run.log | cut -d "(" -f 1) <(cat gold.log | cut -d "(" -f 1) > tmpdiff.log

diff.log : tmpdiff.log mydiff.py
	./mydiff.py > diff.log
