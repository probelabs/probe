class DataProcessor:
    """Processes data items"""

    def __init__(self, name):
        self.name = name
        self.items = []

    def process(self, data):
        """Process a list of data items"""
        return [x * 2 for x in data if x > 0]

    def validate(self, data):
        """Validate data before processing"""
        return all(isinstance(x, (int, float)) for x in data)


class StreamProcessor:
    """Processes streaming data"""

    def __init__(self, buffer_size=1024):
        self.buffer_size = buffer_size

    def process(self, stream):
        """Process a data stream"""
        result = []
        for chunk in stream:
            result.extend(chunk)
        return result

    def flush(self):
        """Flush the internal buffer"""
        pass


def process(items):
    """Top-level process function"""
    return list(map(str, items))
