#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

cd $DIR/node_modules/hecate-ui
ln -nsf .. node_modules
NODE_ENV=prodution parcel build index.html login/index.html --public-url='/admin/' --no-source-maps
cd $DIR
ln -nsf node_modules/hecate-ui/dist
