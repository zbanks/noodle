import gdb
import gdb.printing

class StrPrinter(object):
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

class WordPrinter(object):
    WORD_TUPLE_N = 5 #int(gdb.parse_and_eval("WORD_TUPLE_N"))

    def __init__(self, val):
        self.val = val

    def to_string(self):
        if not self.val:
            return "(null)"
        elif self.val["is_tuple"]:
            tws = self.val["tuple_words"]
            return "{canonical = %s, tuple = {%s}}" % (
                    self.val["canonical"],
                    ", ".join([str(tws[i].dereference()) for i in range(self.WORD_TUPLE_N) if tws[i]]),
                )
        else:
            return "{canonical = %s, original = %s, sorted = %s, value = %s}" % (
                    self.val["canonical"],
                    self.val["original"],
                    self.val["sorted"],
                    self.val["value"],
                )

class WordsetPrinter(object):
    def __init__(self, val):
        self.val = val

    def to_string(self):
        count = int(self.val["words_count"])
        output = "(struct wordset) { name = %s, words_count = %s, words_capacity = %s, is_canonically_sorted = %s" % (
                self.val["name"],
                count,
                self.val["words_capacity"],
                self.val["is_canonically_sorted"],
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
    pp.add_printer("str", "str$", StrPrinter)
    pp.add_printer("word", "word$", WordPrinter)
    pp.add_printer("wordset", "wordset$", WordsetPrinter)
    return pp

gdb.printing.register_pretty_printer(
    gdb.current_objfile(),
    build_pretty_printer())

