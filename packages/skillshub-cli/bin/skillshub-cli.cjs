#!/usr/bin/env node
'use strict'

const { launch } = require('../lib/platform.cjs')

process.exitCode = launch()
