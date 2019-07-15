SHELL := /bin/bash

.PHONY = DIFFED
X=100000

all:
	ln -s ../../.pre-commit ./.git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit

DIFFED : gold.log run.log

	cat run.log | head -n $(X) | cut -d "(" -f 1 | perl -pe 's/IF:[0-9a-f]+ IE:[0-9a-f]+ IME:[0-9a-f]+//ig' | grep '^A' > run.filter.log
	cat gold.log | head -n $(X) | cut -d "(" -f 1 | grep '^A' > gold.filter2.log
	cat gold.log | head -n $(X) | grep '^A' > gold.filter.log
	nice sdiff -d -Z run.filter.log gold.filter2.log   > tmpdiff.log || true
	true

tmpdiff.log gold.filter.log : DIFFED

diff.log : tmpdiff.log mydiff.py gold.filter.log
	nice ./mydiff.py > diff.log
