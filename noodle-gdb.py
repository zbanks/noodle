# This contains pretty-printers for libnoodle data structures for use with GDB

import gdb
import gdb.printing


class WordPrinter(object):
    def __init__(self, val):
        self.val = val

    def to_string(self):
        if not self.val:
            return "(null)"
        small = self.val["small"]
        if small[0]:
            return '"{}"'.format(small.string())
        else:
            return '"{}"'.format(self.val["large"].string())


class WordsetPrinter(object):
    def __init__(self, val):
        self.val = val

    def to_string(self):
        count = int(self.val["words_count"])
        output = "(struct wordset) { .words_count = %s, .words_capacity = %s" % (
            count,
            self.val["words_capacity"],
        )
        if count > 0:
            output += ", \n"
        show_count = min(20, count)

        words = self.val["words"]
        for i in range(show_count):
            if words[i]:
                w = str(words[i].dereference())
            else:
                w = "(null)"
            output += "    {},\n".format(w)
        if show_count < count:
            output += "..."
        output += "}"
        return output


def build_pretty_printer():
    pp = gdb.printing.RegexpCollectionPrettyPrinter("noodle")
    pp.add_printer("word", "word$", WordPrinter)
    pp.add_printer("wordset", "wordset$", WordsetPrinter)
    return pp


gdb.printing.register_pretty_printer(gdb.current_objfile(), build_pretty_printer())
