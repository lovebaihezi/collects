#!/usr/bin/env bun
import { cac } from 'cac';
import prompts from 'prompts';
import { $ } from 'bun';
import { type } from 'arktype';

const cli = cac('services');

const setupCmd = cli.command('[...]', `Helpers for service needed stuff:

  1. Github Action gcloud integration auth
  2. Database connection string in GCP Secret Manager
`).action(async () => {})

cli.help();
cli.parse();
