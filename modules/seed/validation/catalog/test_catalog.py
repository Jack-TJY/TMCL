import unittest
from check import old_rows, expand_tags, check_runtime


class CatalogTest(unittest.TestCase):
    def test_old_alias_is_not_counted_twice(self):
        data = old_rows('1.18', ['CAT target endcity', 'CAT row minecraft:the_end endcity minecraft:end_city'])
        self.assertEqual(data['dimensions']['minecraft:the_end'], {'endcity': ['minecraft:end_city']})

    def test_unassigned_command_is_not_silently_dropped(self):
        with self.assertRaises(ValueError):
            old_rows('1.18', ['CAT target missing'])

    def test_nested_tags_expand_to_unique_targets(self):
        self.assertEqual(expand_tags({'#a':['x','y'], '#b':['#a','x']}, {'x','y'})['#b'], ['x','y'])

    def test_live_check_compares_members_and_dimension_not_only_counts(self):
        data = {'dimensions': {'minecraft:the_end': {'minecraft:end_city': ['minecraft:end_city']}},
                'tags': {'#minecraft:example': ['minecraft:end_city']}}
        rows = ['prefix RUNTIME minecraft:the_end minecraft:end_city',
                'prefix GROUP #minecraft:example minecraft:end_city']
        check_runtime(data, rows)
        with self.assertRaises(ValueError):
            check_runtime(data, [rows[0].replace('the_end', 'overworld'), rows[1]])

    def test_broken_tag_or_cycle_must_fail(self):
        for groups in [{'#a':['missing']}, {'#a':['#a']}]:
            with self.assertRaises(ValueError):
                expand_tags(groups, {'x'})
