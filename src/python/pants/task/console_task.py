# coding=utf-8
# Copyright 2014 Pants project contributors (see CONTRIBUTORS.md).
# Licensed under the Apache License, Version 2.0 (see LICENSE).

from __future__ import absolute_import, division, print_function, unicode_literals

import errno
import os
from contextlib import contextmanager

from pants.base.exceptions import TaskError
from pants.engine.console import LineOriented
from pants.task.task import QuietTaskMixin, Task
from pants.util.dirutil import safe_open
from pants.util.memo import memoized_property


class ConsoleTask(QuietTaskMixin, Task):
  """A task whose only job is to print information to the console.

  ConsoleTasks are not intended to modify build state.
  """

  @classmethod
  def register_options(cls, register):
    super(ConsoleTask, cls).register_options(register)
    register('--sep', default='\\n', metavar='<separator>',
             removal_version='1.18.0.dev2',
             removal_hint='Use `--lines-sep instead.`',
             help='String to use to separate results.')
    register('--output-file', metavar='<path>',
             removal_version='1.18.0.dev2',
             removal_hint='Use `--lines-output-file instead.`',
             help='Write the console output to this file instead.')

  @classmethod
  def subsystem_dependencies(cls):
    return super(ConsoleTask, cls).subsystem_dependencies() + (LineOriented.scoped(cls),)

  def __init__(self, *args, **kwargs):
    super(ConsoleTask, self).__init__(*args, **kwargs)
    self._console_separator = self.lines_options.sep.encode('utf-8').decode('unicode_escape')
    if self.lines_options.output_file:
      try:
        self._outstream = safe_open(os.path.abspath(self.lines_options.output_file), 'wb')
      except IOError as e:
        raise TaskError('Error opening stream {out_file} due to'
                        ' {error_str}'.format(out_file=self.lines_options.output_file, error_str=e))
    else:
      self._outstream = self.context.console_outstream

  @memoized_property
  def lines_options(self):
    lines_options = LineOriented.scoped_instance(self).get_options()
    if not lines_options.is_default('sep') or not lines_options.is_default('output_file'):
      return lines_options
    else:
      return self.get_options()

  @contextmanager
  def _guard_sigpipe(self):
    try:
      yield
    except IOError as e:
      # If the pipeline only wants to read so much, that's fine; otherwise, this error is probably
      # legitimate.
      if e.errno != errno.EPIPE:
        raise e

  def execute(self):
    with self._guard_sigpipe():
      try:
        targets = self.context.targets()
        for value in self.console_output(targets) or tuple():
          self._outstream.write(value.encode('utf-8'))
          self._outstream.write(self._console_separator.encode('utf-8'))
      finally:
        self._outstream.flush()
        if self.lines_options.output_file:
          self._outstream.close()

  def console_output(self, targets):
    raise NotImplementedError('console_output must be implemented by subclasses of ConsoleTask')
