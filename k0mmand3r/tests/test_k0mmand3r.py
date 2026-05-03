# test_k0mmand3r.py

import unittest
import k0mmand3r as k0mmand3r_py


class TestK0mmand3r(unittest.TestCase):
    # def test_verb_parsing(self):
    #     input_str = "/verb --param1=value1 --param2=value2 --tag random content"
    #     result = k0mmand3r_py.parse_kmd_line(input_str)
    #     self.assertIn("verb", result)
    #     self.assertIn("param1", result)
    #     self.assertIn("value1", result)

    def test_verb_parsing(self):
        input_str = "/verb --param1=value1 --param2=value2 --tag random content"
        result = k0mmand3r_py.parse_kmd_line(input_str)
        self.assertEqual(result.verb, "verb")  # Access verb directly
        # Add more assertions for params and content

        # Check parameters (assuming params is a JSON string or similar representation)
        self.assertIn("param1", result.params)
        self.assertEqual(result.params["param1"], "value1")
        self.assertIn("param2", result.params)
        self.assertEqual(result.params["param2"], "value2")
        self.assertIn("tag", result.params)

        # Check content
        self.assertEqual(result.content, "random content")

    def test_content_only(self):
        input_str = "this is just content, no verb!"
        result = k0mmand3r_py.parse_kmd_line(input_str)
        self.assertIsNone(result.verb)  # Access verb directly
        self.assertEqual(
            result.content, "this is just content, no verb!"
        )  # Access content directly

        # Check that verb is None
        self.assertIsNone(result.verb)

        # Check content
        self.assertEqual(result.content, "this is just content, no verb!")

        # Check that params is None or empty
        self.assertIsNone(
            result.params
        )  # Adjust if 'params' is expected to be empty string or similar

    # def test_verb_parsing(self):
    #     input_str = "/verb --param1=value1 --param2=value2 --tag random content"
    #     result = k0mmand3r_py.parse_kmd_line(input_str)
    #     self.assertEqual(result.verb(), "verb")
    #     # Add more assertions as needed, e.g., checking params and content

    # # def test_content_only(self):
    # #     input_str = "this is just content, no verb!"
    # #     result = k0mmand3r_py.parse_kmd_line(input_str)
    # #     self.assertIn("content", result)
    # #     self.assertNotIn("verb", "")

    # def test_content_only(self):
    #     input_str = "this is just content, no verb!"
    #     result = k0mmand3r_py.parse_kmd_line(input_str)
    #     self.assertIsNone(result.verb())
    #     self.assertEqual(result.content(), input_str)

    ## no exceptions can be raised yet.
    # def test_error_handling(self):
    #     input_str = "/invalid --param"
    #     with self.assertRaises(Exception):
    #         k0mmand3r_py.parse_kmd_line(input_str)


# More tests can be added as needed...

if __name__ == "__main__":
    unittest.main()
