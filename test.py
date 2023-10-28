from typing import List
import unittest
from pprint import pprint
from index_extensions.index_tasks import find_date_with_marker
        
class TestFindDueDate(unittest.TestCase):
    def test_find_due_date(self):
        # Test with no due date
        content = "This is a task that must be done"
        expected_output = (content, None)
        assert find_date_with_marker("📅", content) == expected_output

        # Test with due date
        content = "This is a task 📅 2021-10-10 that must be done"
        expected_output = ("This is a task that must be done", "2021-10-10")
        assert find_date_with_marker("📅", content) == expected_output

        # Test with due date at the beginning
        content = "📅 2021-10-10 This is a task that must be done"
        expected_output = ("This is a task that must be done", "2021-10-10")
        assert find_date_with_marker("📅", content) == expected_output

        # Test with due date at the end
        content = "This is a task that must be done 📅 2021-10-10"
        expected_output = ("This is a task that must be done", "2021-10-10")
        assert find_date_with_marker("📅", content) == expected_output

        # Test with multiple due dates
        content = "This is a task 📅 2021-12-12 that must be done 📅 2021-10-11"
        expected_output = ("This is a task that must be done 📅 2021-10-11", "2021-12-12")
        assert find_date_with_marker("📅", content) == expected_output


if __name__ == "__main__":
    unittest.main()
