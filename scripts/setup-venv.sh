#!/bin/sh

venv_name=sp-venv
python3 -m venv $venv_name
source $venv_name/bin/activate

pip install -r requirements.txt
