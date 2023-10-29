.PHONY: test
test:
	jupyter nbconvert --to notebook --execute test.ipynb --inplace