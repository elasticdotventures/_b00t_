import Mocha from 'mocha';
import { glob } from 'glob';
import { promisify } from 'util';
import * as path from 'path';

const globAsync = promisify(glob);

export function run(): Promise<void> {
  const mocha = new Mocha({ ui: 'tdd', color: true });
  const testsRoot = path.resolve(__dirname, '..');

  return new Promise((c, e) => {
    globAsync('suite/**/*.test.js', { cwd: testsRoot }).then((files: string[]) => {
      files.forEach((f: string) => mocha.addFile(path.resolve(testsRoot, f)));
      try {
        mocha.run((failures: number) => {
          if (failures > 0) {
            e(new Error(`${failures} tests failed.`));
          } else {
            c();
          }
        });
      } catch (err) {
        e(err);
      }
    }).catch(err => e(err));
  });
}