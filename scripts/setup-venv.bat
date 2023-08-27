set venv_name=sp-venv
python.exe -m venv %venv-name%

%venv-name%\Scripts\activate.bat

pip install -r requirements.txt
