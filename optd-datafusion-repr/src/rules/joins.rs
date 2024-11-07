use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use optd_core::nodes::{PlanNode, PlanNodeOrGroup};
use optd_core::optimizer::Optimizer;
use optd_core::rules::{Rule, RuleMatcher};

use super::macros::{define_impl_rule, define_rule};
use crate::plan_nodes::{
    rewrite_column_refs, ArcDfPlanNode, BinOpPred, BinOpType, ColumnRefPred, ConstantPred,
    ConstantType, DfNodeType, DfReprPlanNode, DfReprPredNode, JoinType, ListPred, LogOpType,
    LogicalEmptyRelation, LogicalJoin, LogicalProjection, PhysicalHashJoin,
};
use crate::properties::schema::{Schema, SchemaPropertyBuilder};
use crate::OptimizerExt;

define_rule!(
    InnerCrossJoinRule,
    apply_inner_cross_join,
    (DfNodeType::Join(JoinType::Cross), left, right)
);

fn apply_inner_cross_join(
    _: &impl Optimizer<DfNodeType>,
    binding: ArcDfPlanNode,
) -> Vec<PlanNodeOrGroup<DfNodeType>> {
    let join = LogicalJoin::from_plan_node(binding).unwrap();
    let node = LogicalJoin::new_unchecked(join.left(), join.right(), join.cond(), JoinType::Inner);
    vec![node.into_plan_node().into()]
}

// A join B -> B join A
define_rule!(
    JoinCommuteRule,
    apply_join_commute,
    (DfNodeType::Join(JoinType::Inner), left, right)
);

fn apply_join_commute(
    optimizer: &impl Optimizer<DfNodeType>,
    binding: ArcDfPlanNode,
) -> Vec<PlanNodeOrGroup<DfNodeType>> {
    let join = LogicalJoin::from_plan_node(binding).unwrap();
    let left = join.left();
    let right = join.right();
    let left_schema = optimizer.get_schema_of(left.clone());
    let right_schema = optimizer.get_schema_of(right.clone());
    let cond = rewrite_column_refs(join.cond(), &mut |idx| {
        Some(if idx < left_schema.len() {
            idx + right_schema.len()
        } else {
            idx - left_schema.len()
        })
    })
    .unwrap();
    let node = LogicalJoin::new_unchecked(right, left, cond, JoinType::Inner);
    let mut proj_expr = Vec::with_capacity(left_schema.len() + right_schema.len());
    for i in 0..left_schema.len() {
        proj_expr.push(ColumnRefPred::new(right_schema.len() + i).into_pred_node());
    }
    for i in 0..right_schema.len() {
        proj_expr.push(ColumnRefPred::new(i).into_pred_node());
    }
    let node = LogicalProjection::new(node.into_plan_node(), ListPred::new(proj_expr));
    vec![node.into_plan_node().into()]
}

// define_rule!(
//     EliminateJoinRule,
//     apply_eliminate_join,
//     (Join(JoinType::Inner), left, right)
// );

// /// Eliminate logical join with constant predicates
// /// True predicates becomes CrossJoin (not yet implemented)
// #[allow(unused_variables)]
// fn apply_eliminate_join(
//     optimizer: &impl Optimizer<DfNodeType>,
//     binding: ArcDfPlanNode,
// ) -> Vec<PlanNodeOrGroup<DfNodeType>> {
//     if let DfNodeType::Constant(const_type) = cond.typ {
//         if const_type == ConstantType::Bool {
//             if let Some(data) = cond.data {
//                 if data.as_bool() {
//                     // change it to cross join if filter is always true
//                     let node = LogicalJoin::new(
//                         DfReprPlanNode::from_group(left.into()),
//                         DfReprPlanNode::from_group(right.into()),
//                         ConstantPred::bool(true).into_expr(),
//                         JoinType::Cross,
//                     );
//                     return vec![node.into_rel_node().as_ref().clone()];
//                 } else {
//                     // No need to handle schema here, as all exprs in the same group
//                     // will have same logical properties
//                     let mut left_fields = optimizer
//                         .get_property::<SchemaPropertyBuilder>(Arc::new(left.clone()), 0)
//                         .fields;
//                     let right_fields = optimizer
//                         .get_property::<SchemaPropertyBuilder>(Arc::new(right.clone()), 0)
//                         .fields;
//                     left_fields.extend(right_fields);
//                     let new_schema = Schema {
//                         fields: left_fields,
//                     };
//                     let node = LogicalEmptyRelation::new(false, new_schema);
//                     return vec![node.into_rel_node().as_ref().clone()];
//                 }
//             }
//         }
//     }
//     vec![]
// }

// // (A join B) join C -> A join (B join C)
// define_rule!(
//     JoinAssocRule,
//     apply_join_assoc,
//     (
//         Join(JoinType::Inner),
//         (Join(JoinType::Inner), a, b, [cond1]),
//         c,
//         [cond2]
//     )
// );

// fn apply_join_assoc(
//     optimizer: &impl Optimizer<DfNodeType>,
//     JoinAssocRulePicks {
//         a,
//         b,
//         c,
//         cond1,
//         cond2,
//     }: JoinAssocRulePicks,
// ) -> Vec<PlanNodeOrGroup<DfNodeType>> {
//     let a_schema = optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(a.clone()), 0);
//     let _b_schema = optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(b.clone()), 0);
//     let _c_schema = optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(c.clone()), 0);

//     let cond2 = Expr::from_rel_node(cond2.into()).unwrap();

//     let Some(cond2) = cond2.rewrite_column_refs(&mut |idx| {
//         if idx < a_schema.len() {
//             None
//         } else {
//             Some(idx - a_schema.len())
//         }
//     }) else {
//         return vec![];
//     };

//     let node = PlanNode {
//         typ: DfNodeType::Join(JoinType::Inner),
//         children: vec![
//             a.into(),
//             PlanNode {
//                 typ: DfNodeType::Join(JoinType::Inner),
//                 children: vec![b.into(), c.into(), cond2.into_rel_node()],
//             }
//             .into(),
//             cond1.into(),
//         ],
//     };
//     vec![node]
// }

// define_impl_rule!(
//     HashJoinRule,
//     apply_hash_join,
//     (Join(JoinType::Inner), left, right, [cond])
// );

// fn apply_hash_join(
//     optimizer: &impl Optimizer<DfNodeType>,
//     HashJoinRulePicks { left, right, cond }: HashJoinRulePicks,
// ) -> Vec<PlanNodeOrGroup<DfNodeType>> {
//     match cond.typ {
//         DfNodeType::BinOp(BinOpType::Eq) => {
//             let left_schema =
//                 optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(left.clone()), 0);
//             // let right_schema =
//             //     optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(right.clone()), 0);
//             let op = BinOpPred::from_rel_node(Arc::new(cond.clone())).unwrap();
//             let left_expr = op.left_child();
//             let right_expr = op.right_child();
//             let Some(mut left_expr) = ColumnRefPred::from_rel_node(left_expr.into_rel_node())
//             else {
//                 return vec![];
//             };
//             let Some(mut right_expr) = ColumnRefPred::from_rel_node(right_expr.into_rel_node())
//             else {
//                 return vec![];
//             };
//             let can_convert = if left_expr.index() < left_schema.len()
//                 && right_expr.index() >= left_schema.len()
//             {
//                 true
//             } else if right_expr.index() < left_schema.len()
//                 && left_expr.index() >= left_schema.len()
//             {
//                 (left_expr, right_expr) = (right_expr, left_expr);
//                 true
//             } else {
//                 false
//             };

//             if can_convert {
//                 let right_expr = ColumnRefPred::new(right_expr.index() - left_schema.len());
//                 let node = PhysicalHashJoin::new(
//                     DfReprPlanNode::from_group(left.into()),
//                     DfReprPlanNode::from_group(right.into()),
//                     ListPred::new(vec![left_expr.into_expr()]),
//                     ListPred::new(vec![right_expr.into_expr()]),
//                     JoinType::Inner,
//                 );
//                 return vec![node.into_rel_node().as_ref().clone()];
//             }
//         }
//         DfNodeType::LogOp(LogOpType::And) => {
//             // currently only support consecutive equal queries
//             let mut is_consecutive_eq = true;
//             for child in cond.children.clone() {
//                 if let DfNodeType::BinOp(BinOpType::Eq) = child.typ {
//                     continue;
//                 } else {
//                     is_consecutive_eq = false;
//                     break;
//                 }
//             }
//             if !is_consecutive_eq {
//                 return vec![];
//             }

//             let left_schema =
//                 optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(left.clone()), 0);
//             let mut left_exprs = vec![];
//             let mut right_exprs = vec![];
//             for child in cond.children {
//                 let bin_op = BinOpPred::from_rel_node(child.clone()).unwrap();
//                 let left_expr: Expr = bin_op.left_child();
//                 let right_expr = bin_op.right_child();
//                 let Some(mut left_expr) = ColumnRefPred::from_rel_node(left_expr.into_rel_node())
//                 else {
//                     return vec![];
//                 };
//                 let Some(mut right_expr) = ColumnRefPred::from_rel_node(right_expr.into_rel_node())
//                 else {
//                     return vec![];
//                 };
//                 let can_convert = if left_expr.index() < left_schema.len()
//                     && right_expr.index() >= left_schema.len()
//                 {
//                     true
//                 } else if right_expr.index() < left_schema.len()
//                     && left_expr.index() >= left_schema.len()
//                 {
//                     (left_expr, right_expr) = (right_expr, left_expr);
//                     true
//                 } else {
//                     false
//                 };
//                 if !can_convert {
//                     return vec![];
//                 }
//                 let right_expr = ColumnRefPred::new(right_expr.index() - left_schema.len());
//                 right_exprs.push(right_expr.into_expr());
//                 left_exprs.push(left_expr.into_expr());
//             }

//             let node = PhysicalHashJoin::new(
//                 DfReprPlanNode::from_group(left.into()),
//                 DfReprPlanNode::from_group(right.into()),
//                 ListPred::new(left_exprs),
//                 ListPred::new(right_exprs),
//                 JoinType::Inner,
//             );
//             return vec![node.into_rel_node().as_ref().clone()];
//         }
//         _ => {}
//     }
//     if let DfNodeType::BinOp(BinOpType::Eq) = cond.typ {
//         let left_schema =
//             optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(left.clone()), 0);
//         // let right_schema =
//         //     optimizer.get_property::<SchemaPropertyBuilder>(Arc::new(right.clone()), 0);
//         let op = BinOpPred::from_rel_node(Arc::new(cond.clone())).unwrap();
//         let left_expr = op.left_child();
//         let right_expr = op.right_child();
//         let Some(mut left_expr) = ColumnRefPred::from_rel_node(left_expr.into_rel_node()) else {
//             return vec![];
//         };
//         let Some(mut right_expr) = ColumnRefPred::from_rel_node(right_expr.into_rel_node()) else {
//             return vec![];
//         };
//         let can_convert = if left_expr.index() < left_schema.len()
//             && right_expr.index() >= left_schema.len()
//         {
//             true
//         } else if right_expr.index() < left_schema.len() && left_expr.index() >= left_schema.len() {
//             (left_expr, right_expr) = (right_expr, left_expr);
//             true
//         } else {
//             false
//         };

//         if can_convert {
//             let right_expr = ColumnRefPred::new(right_expr.index() - left_schema.len());
//             let node = PhysicalHashJoin::new(
//                 DfReprPlanNode::from_group(left.into()),
//                 DfReprPlanNode::from_group(right.into()),
//                 ListPred::new(vec![left_expr.into_expr()]),
//                 ListPred::new(vec![right_expr.into_expr()]),
//                 JoinType::Inner,
//             );
//             return vec![node.into_rel_node().as_ref().clone()];
//         }
//     }
//     vec![]
// }
